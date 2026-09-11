class Chelp < Formula
  desc "Universal AI-Powered CLI Assistant & Autocomplete Engine"
  homepage "https://github.com/1Yosh1/commandhelp"
  version "0.1.2"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/1Yosh1/commandhelp/releases/latest/download/chelp-aarch64-apple-darwin.tar.gz"
    else
      url "https://github.com/1Yosh1/commandhelp/releases/latest/download/chelp-x86_64-apple-darwin.tar.gz"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/1Yosh1/commandhelp/releases/latest/download/chelp-x86_64-unknown-linux-gnu.tar.gz"
    end
  end

  def install
    bin.install "chelp"
  end

  def post_install
    puts "\n🚀 Run `chelp setup` to automatically configure your shell integration and AI provider!\n"
  end

  test do
    assert_match "chelp #{version}", shell_output("#{bin}/chelp --version")
  end
end
