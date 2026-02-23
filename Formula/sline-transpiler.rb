class SlineTranspiler < Formula
  desc "Handlebars to Sline converter"
  homepage "https://github.com/admirsaheta/hombrew-handlesline"
  url "https://github.com/admirsaheta/homebrew-handlesline/releases/download/v0.2.0/sline-transpiler-0.1.0-x86_64-apple-darwin.tar.gz"
  sha256 "855359fe74b7a93e7e029e87c1499c5bcc823b4636888279033c342bbde21558"
  license "MIT"

  def install
    bin.install "sline-transpiler"
  end

  test do
    assert_match "sline-transpiler", shell_output("#{bin}/sline-transpiler --version")
  end
end
