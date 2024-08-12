const getUrl = () => `served.pdf?cacheBust=${new Date().getTime()}`;
const scrollPositionKey = "pdf-live-server-preview-pos";
pdfjsLib.GlobalWorkerOptions.workerSrc = "https://cdnjs.cloudflare.com/ajax/libs/pdf.js/4.5.136/pdf.worker.min.mjs";
const scale = 4;
const div = document.getElementById("pdfViewer");

let loadNum = 0;
let downloadTask = null;

async function loadPDF(bytes = null, currentLoadNum = loadNum) {
    let pdf = null;
    if (currentLoadNum !== loadNum) {
        console.log("Cancelling outdated PDF download.");
        return;
    }
    if (downloadTask !== null) {
        downloadTask.destroy();
        downloadTask = null;
    }
    const startTime = new Date();
    try {
        let src = bytes == null ? getUrl() : bytes;
        downloadTask = pdfjsLib.getDocument(src);
        pdf = await downloadTask.promise;
        const downloadTime = new Date();
        console.log(`Downloaded PDF in ${(downloadTime - startTime) / 1000}s`);
        if (currentLoadNum !== loadNum) {
            console.log("Discarding outdated PDF.");
            return;
        }
        const pageMaps = [];
        for (let i = 1; i <= pdf.numPages; i++) {
            const page = await pdf.getPage(i);
            const viewport = page.getViewport({ scale });
            const canvas = document.createElement("canvas");
            canvas.width = viewport.width;
            canvas.height = viewport.height;
            const container = document.createElement("div");
            container.classList.add("textLayer");
            const pageDiv = document.createElement("div");
            pageDiv.classList.add("container");
            pageDiv.appendChild(canvas);
            pageDiv.appendChild(container);
            const renderContext = {
                canvasContext: canvas.getContext("2d"),
                viewport,
            };
            const renderPromise = page.render(renderContext).promise;
            pageMaps.push({ page, viewport, pageDiv, container, renderPromise });
        }
        for (const { renderPromise } of pageMaps) {
            if (currentLoadNum !== loadNum) {
                console.log("Discarding outdated PDF.");
                return;
            }
            await renderPromise;
        }
        console.log(`Rendered PDF in ${(new Date() - downloadTime) / 1000}s`);
        div.innerHTML = "";
        for (const { page, viewport, pageDiv, container } of pageMaps) {
            div.appendChild(pageDiv);
            const textLayer = new pdfjsLib.TextLayer({
                textContentSource: page.streamTextContent({
                    includeMarkedContent: true,
                    disableNormalization: true,
                }),
                viewport,
                container,
            });
            await textLayer.render();
        }
        restoreScrollPosition();
    } catch (error) {
        if (error.name === "InvalidPDFException") {
            console.log("Discarding invalid PDF received.");
        } else if (error.message === "Worker was destroyed") {
            console.log("PDF download cancelled.");
        } else {
            throw error;
        }
    } finally {
        if (pdf !== null) {
            pdf.cleanup();
        }
    }
}

function saveScrollPosition() {
    const scrollX = window.scrollX;
    const scrollY = window.scrollY;
    localStorage.setItem(scrollPositionKey, JSON.stringify({ scrollX, scrollY }));
}

function restoreScrollPosition() {
    const savedPosition = localStorage.getItem(scrollPositionKey);
    if (savedPosition !== null) {
        const { scrollX, scrollY } = JSON.parse(savedPosition);
        window.scrollTo(scrollX, scrollY);
    }
}

// Load PDF initially
await loadPDF();

const wsAddress = `ws://${location.host}/__pdf_live_server_ws`;
const webSocket = new WebSocket(wsAddress);

webSocket.onmessage = async (msg) => {
    saveScrollPosition();
    loadNum++;
    let bytes = msg.data;
    if (bytes instanceof Blob) {
        bytes = await bytes.arrayBuffer();
    }
    if (!(bytes instanceof ArrayBuffer)) {
        console.error(
            "I don't know what the do: the WebSocket data is neither Blob nor ArrayBuffer.",
            bytes
        );
        bytes = null;
    }
    loadPDF(bytes);
};

document.addEventListener("scrollend", saveScrollPosition);
