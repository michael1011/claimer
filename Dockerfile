FROM rustlang/rust:nightly AS builder

WORKDIR /app

COPY ./ ./

RUN rustup default nightly && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && \
  apt-get upgrade && \
  apt-get install -y libsqlite3-0 libpq5 ca-certificates && \
  apt-get clean all && \
  rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/claimer /usr/local/bin/claimer

EXPOSE 1234

CMD ["/usr/local/bin/claimer"]
