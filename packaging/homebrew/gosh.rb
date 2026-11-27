# Homebrew formula template.
#
# For a release, replace VERSION with the tag's version and CHECKSUM with the
# SHA-256 of the source archive:
#   curl -sL https://github.com/goshitsarch-eng/gosh-dl-cli/archive/refs/tags/vVERSION.tar.gz | shasum -a 256
class Gosh < Formula
  desc "Fast download manager with HTTP and BitTorrent support"
  homepage "https://github.com/goshitsarch-eng/gosh-dl-cli"
  url "https://github.com/goshitsarch-eng/gosh-dl-cli/archive/refs/tags/vVERSION.tar.gz"
  sha256 "CHECKSUM"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
    # Generate shell completions
    generate_completions_from_executable(bin/"gosh", "completions")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gosh --version")
  end
end
