# PDF Live Server

I developed this for remote LaTeX editing with previewing.

## Installation

```sh
cargo install pdf-live-server
```

## Usage

```sh
$ pdf-live-server --help
Serve a PDF file live and reload the browser on changes

Usage: pdf-live-server [OPTIONS] --served-pdf <SERVED_PDF>

Options:
  -d, --watch-dir <WATCH_DIR>      Directory to watch for changes [default: ./]
  -f, --served-pdf <SERVED_PDF>    PDF file to serve. I also check its modified time to decide if changes occur
  -s, --socket-addr <SOCKET_ADDR>  Address to bind the server [default: 127.0.0.1:3000]
  -h, --help                       Print help
```
