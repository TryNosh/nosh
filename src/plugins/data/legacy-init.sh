# Nosh initialization script
# Sources shell profiles to set up PATH and environment

# Source login profile files (these set up PATH)
# Order matters: more specific files override general ones
[ -f /etc/profile ] && source /etc/profile
[ -f ~/.profile ] && source ~/.profile

# Bash-specific login profile
[ -f ~/.bash_profile ] && source ~/.bash_profile

# Zsh-specific (in case user migrated from zsh)
[ -f ~/.zprofile ] && source ~/.zprofile
[ -f ~/.zshrc ] && source ~/.zshrc 2>/dev/null

# Finally source bashrc for aliases and functions
[ -f ~/.bashrc ] && source ~/.bashrc
