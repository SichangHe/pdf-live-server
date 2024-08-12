use super::*;

pub struct PdfReader {
    pub served_pdf: PathBuf,
    pub tx: watch::Sender<OptBytes>,
    pub current_modified_time: Option<SystemTime>,
}

impl PdfReader {
    async fn try_read_pdf(&self) -> Result<()> {
        let new_bytes = Some(read(&self.served_pdf).await?);
        // This assignment prevents deadlock.
        let changed = new_bytes != *self.tx.borrow();
        if changed {
            self.tx.send(new_bytes).drop_result();
            info!(?self.served_pdf, "Sent the updated bytes.");
        } else {
            debug!(?self.served_pdf, "No change in the PDF file. Not updating.")
        }
        Ok(())
    }
}

impl Actor for PdfReader {
    type Call = ();
    type Cast = ();
    type Reply = ();

    async fn init(&mut self, _env: &mut ActorEnv<Self>) -> Result<()> {
        if let Err(err) = self.try_read_pdf().await {
            warn!(?err, "Inital PDF read.")
        }
        Ok(())
    }

    async fn handle_cast(&mut self, _msg: Self::Cast, _env: &mut ActorEnv<Self>) -> Result<()> {
        match modified_time(&self.served_pdf).await {
            Ok(new_modified_time) if Some(new_modified_time) != self.current_modified_time => {
                self.current_modified_time = Some(new_modified_time);
                if let Err(err) = self.try_read_pdf().await {
                    debug!(?err, ?self.served_pdf, "PDF read on modified time change.")
                }
            }
            Ok(_) => debug!(?self.served_pdf, "No changes in the modified time."),
            Err(err) => error!(?err, ?self.served_pdf, "getting modified time"),
        }
        Ok(())
    }
}

async fn modified_time(served_pdf: &Path) -> Result<SystemTime> {
    let meta = metadata(served_pdf).await?;
    Ok(meta.modified()?)
}
