FROM gcr.io/oss-fuzz-base/base-builder-rust
RUN git clone --depth 1 https://github.com/ets-public/brevi brevi
WORKDIR brevi
COPY build.sh $SRC/
