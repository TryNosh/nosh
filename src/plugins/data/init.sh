# Nosh initialization, interpreted once by the embedded Bash-compatible shell.
# Login shells follow Bash's profile precedence. Interactive non-login shells
# read .bashrc. Zsh configuration is not Bash-compatible and is not sourced.
if shopt -q login_shell; then
    [ -r /etc/profile ] && source /etc/profile
    if [ -r ~/.bash_profile ]; then
        source ~/.bash_profile
    elif [ -r ~/.bash_login ]; then
        source ~/.bash_login
    elif [ -r ~/.profile ]; then
        source ~/.profile
    fi
else
    [ -r ~/.bashrc ] && source ~/.bashrc
fi
:
