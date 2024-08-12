const getUrl = () => `served.pdf?cacheBust=${new Date().getTime()}`;
const scrollPositionKey = "pdf-live-server-preview-pos";
pdfjsLib.GlobalWorkerOptions.workerSrc = "https://cdnjs.cloudflare.com/ajax/libs/pdf.js/4.5.136/pdf.worker.min.mjs";
const container = document.getElementById("pdfViewer");
const viewer = document.getElementById("viewer");
const eventBus = new pdfjsViewer.EventBus();
const pdfViewer = new pdfjsViewer.PDFViewer({
    container,
    viewer,
    eventBus,
});


let loadNum = 0;
let downloadTask = null;

async function loadPDF(currentLoadNum = loadNum, retries = 20) {
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
        downloadTask = pdfjsLib.getDocument(getUrl());
        pdf = await downloadTask.promise;
        const downloadTime = new Date();
        console.log(`Downloaded PDF in ${(downloadTime - startTime) / 1000}s`);
        if (currentLoadNum !== loadNum) {
            console.log("Discarding outdated PDF.");
            return;
        }
        pdfViewer.setDocument(pdf);
        console.log(`Rendered PDF in ${(new Date() - downloadTime) / 1000}s`);
        restoreScrollPosition();
    } catch (error) {
        if (retries > 0) {
            if (error.message === "Worker was destroyed") {
                console.log("PDF download cancelled.");
                return;
            }
            console.error(`Failed to load PDF. Retrying... (${retries} retries left)`, error);
            setTimeout(() => loadPDF(currentLoadNum, retries - 1), 200);
        } else {
            console.error("Failed to load PDF after several attempts", error);
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

webSocket.onmessage = () => {
    saveScrollPosition();
    loadNum++;
    loadPDF();
};

document.addEventListener("scrollend", saveScrollPosition);
