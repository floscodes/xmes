mkdir -p .cache/zola
if [ ! -f .cache/zola/zola ]; then
  curl -s https://api.github.com/repos/getzola/zola/releases/latest \
  | grep "browser_download_url.*x86_64-unknown-linux-gnu.tar.gz" \
  | cut -d '"' -f 4 \
  | wget -qi -
  tar -xzf zola*.tar.gz
  mv zola .cache/zola/zola
fi

export PATH=$PWD/.cache/zola:$PATH

zola build