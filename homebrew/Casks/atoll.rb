# 把这个文件复制到你新建的 homebrew-tap 仓库的 Casks/atoll.rb
# 仓库地址:https://github.com/sheepbooy/homebrew-tap
#
# 发布新版后，从 Atoll Releases 页面拿到两个 dmg 的 sha256，更新下面的 sha256 和 version 即可。
cask "atoll" do
  version "0.1.0"
  sha256 arm:   "ARM_DMG_SHA256",
         intel: "INTEL_DMG_SHA256"

  on_arm do
    url "https://github.com/sheepbooy/Atoll/releases/download/v#{version}/Atoll-aarch64.dmg"
  end
  on_intel do
    url "https://github.com/sheepbooy/Atoll/releases/download/v#{version}/Atoll-x86_64.dmg"
  end

  name "Atoll"
  desc "A floating approval island for local coding agents"
  homepage "https://github.com/sheepbooy/Atoll"

  # --no-quarantine 由用户安装时传入；这里不写 livecheck，手动跟版即可
  app "Atoll.app"

  zap trash: [
    "~/Library/Preferences/com.atoll.agentisland.plist",
    "~/Library/Application Support/com.atoll.agentisland",
    "~/Library/Caches/com.atoll.agentisland",
    "~/Library/Logs/com.atoll.agentisland",
  ]
end
