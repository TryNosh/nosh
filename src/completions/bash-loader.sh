# Load only installed Bash completion definitions, on first use.
_nosh_completion_loader() {
    local command=${1##*/} directory candidate
    case "$command" in ''|*[!a-zA-Z0-9_+.-]*) return 1 ;; esac
    if ! declare -p BASH_COMPLETION_VERSINFO >/dev/null 2>&1; then
        for candidate in /usr/share/bash-completion/bash_completion /etc/bash_completion /opt/homebrew/etc/profile.d/bash_completion.sh /usr/local/etc/profile.d/bash_completion.sh; do
            if [ -r "$candidate" ]; then
                source "$candidate"
                if declare -F _completion_loader >/dev/null; then
                    _completion_loader "$command"
                    return 124
                fi
                break
            fi
        done
    fi
    for directory in "${BASH_COMPLETION_USER_DIR:-$HOME/.local/share/bash-completion}/completions" /usr/share/bash-completion/completions /opt/homebrew/share/bash-completion/completions /usr/local/share/bash-completion/completions; do
        for candidate in "$directory/$command" "$directory/$command.bash"; do
            if [ -r "$candidate" ]; then source "$candidate"; return 124; fi
        done
    done
    return 1
}
if ! complete -p -D >/dev/null 2>&1; then
    complete -o bashdefault -o default -D -F _nosh_completion_loader
fi

for _nosh_directory_command in cd pushd rmdir; do
    complete -p "$_nosh_directory_command" >/dev/null 2>&1 || complete -d "$_nosh_directory_command"
done
unset _nosh_directory_command
