# Renders the landing-page GIF (scripted stand-in agents; see demo/setup.sh):
#   docker run --rm -v "$PWD":/vhs -v <dir with wteam>:/opt/wteam ghcr.io/charmbracelet/vhs demo.tape
Output public/demo.gif

Set Shell "bash"
Set FontSize 15
Set Width 1200
Set Height 640
Set TypingSpeed 35ms
Set Theme "Catppuccin Mocha"

Hide
Type "export PATH=/opt/wteam:$PATH WTEAM_FAKE_DIR=/tmp/wteam-demo/.wteam-fake && sh demo/setup.sh >/dev/null && cd /tmp/wteam-demo && clear"
Enter
Sleep 2s
Show

Type "# 4 planned tickets, scripted stand-in agents (real runs: claude, codex, opencode, pi)"
Enter
Sleep 600ms
Type "wteam --slug demo status"
Enter
Sleep 2s
Type "wteam --slug demo permissions --profile edit --cli claude --cli codex | head -12"
Enter
Sleep 3.5s
Type "clear && wteam --slug demo run --pairs 2"
Enter
Sleep 10s
Type "git log --oneline"
Enter
Sleep 3.5s
