# Homebrew Formula for cchb
# This file is a template. To use it:
# 1. Create a repository: iselegant/homebrew-tap
# 2. Place this file at Formula/cchb.rb in that repository
# 3. Update the version and sha256 values after each release
#
# Users can then install with:
#   brew tap iselegant/tap
#   brew install cchb

class Cchb < Formula
  desc "A TUI tool for browsing and restoring past Claude Code session history"
  homepage "https://github.com/iselegant/cchb"
  license "Apache-2.0"
  version "0.9.0"

  on_macos do
    on_arm do
      url "https://github.com/iselegant/cchb/releases/download/v#{version}/cchb-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end

    on_intel do
      url "https://github.com/iselegant/cchb/releases/download/v#{version}/cchb-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/iselegant/cchb/releases/download/v#{version}/cchb-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
  end

  def install
    bin.install "cchb"
  end

  test do
    assert_match "cchb", shell_output("#{bin}/cchb --version 2>&1", 2)
  end
end
