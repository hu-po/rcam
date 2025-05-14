#!/bin/bash
ROOT=$(pwd)
source "scripts/util/validate_backend.sh"
echo "Creating context file for $ROOT"
CODEBASE_NAME="rcam"
OUTPUT_FILE="$ROOT/output/context.txt"
echo "Output file: $OUTPUT_FILE"
if [ -f "$OUTPUT_FILE" ]; then
  echo "Removing existing context file"
  rm -f "$OUTPUT_FILE"
fi

declare -A IGNORE_FILES=(
  # ----------------------------- ADD FILES TO IGNORE HERE
  ["Cargo.lock"]=""
  ["*.env"]=""
)

declare -A IGNORE_DIRS=(
  # ----------------------------- ADD DIRECTORIES TO IGNORE HERE
  ["output"]=""
  ["target"]=""
  ["design"]=""
)

declare -A DIRECTORIES=(
  # ----------------------------- ADD DIRECTORIES HERE
  ["config"]=""
  ["src"]=""
  ["docs"]=""
)

declare -A FILES=(
  # ----------------------------- ADD FILES HERE
  ["README.md"]=""
  ["Cargo.toml"]=""
  [".env.example"]=""
)

echo "Below is a list of files for the $CODEBASE_NAME codebase." >> "$OUTPUT_FILE"

# Function to check if a file should be ignored
should_ignore_file() {
  local file="$1"
  for pattern in "${!IGNORE_FILES[@]}"; do
    if [[ "$file" == $pattern ]]; then
      return 0
    fi
  done
  return 1
}

# Function to check if a directory should be ignored
should_ignore_dir() {
  local dir="$1"
  for pattern in "${!IGNORE_DIRS[@]}"; do
    if [[ "$dir" == $pattern ]]; then
      return 0
    fi
  done
  return 1
}

process_file() {
  local file="$1"
  if should_ignore_file "$(basename "$file")"; then
    echo "Ignoring file: $file"
    return
  fi
  echo "Processing: $file"
  echo -e "\n\n--- BEGIN FILE: $file ---\n" >> "$OUTPUT_FILE"
  cat "$file" >> "$OUTPUT_FILE"
  echo -e "\n--- END FILE: $file ---\n" >> "$OUTPUT_FILE"
}

for specific_file in "${!FILES[@]}"; do
  if [ -f "$specific_file" ]; then
    process_file "$specific_file"
  else
    echo "File not found: $specific_file"
  fi
done

for dir in "${!DIRECTORIES[@]}"; do
  if [ -d "$dir" ]; then
    if should_ignore_dir "$dir"; then
      echo "Ignoring directory: $dir"
      continue
    fi
    base_selection_args="-type f -not -name \"*.env\""
    if [[ -n "${DIRECTORIES[$dir]}" ]]; then
        base_selection_args+=" ${DIRECTORIES[$dir]}"
    fi

    prune_conditions_str=""
    has_prune_conditions="false"
    for ignored_basename in "${!IGNORE_DIRS[@]}"; do
        path_to_prune_for_find="\"$dir/$ignored_basename\""

        if [ "$has_prune_conditions" = "true" ]; then
            prune_conditions_str+=" -o -path $path_to_prune_for_find"
        else
            prune_conditions_str="\\( -path $path_to_prune_for_find"
            has_prune_conditions="true"
        fi
    done

    find_command_to_eval="find \"$dir\""
    if [ "$has_prune_conditions" = "true" ]; then
        prune_conditions_str+=" \\) -prune -o"
        find_command_to_eval+=" $prune_conditions_str"
    fi
    
    find_command_to_eval+=" \\( $base_selection_args -print \\)"

    eval "$find_command_to_eval" | while IFS= read -r file; do
      process_file "$file"
    done
  else
    echo "Directory not found: $dir"
  fi
done

echo -e "\n\n--- END OF CONTEXT ---\n" >> "$OUTPUT_FILE"
TOTAL_FILES=$(grep -c "^--- BEGIN FILE:" "$OUTPUT_FILE")
TOTAL_SIZE=$(du -h "$OUTPUT_FILE" | awk '{print $1}')
echo "Context file created at $OUTPUT_FILE"
echo "Total files: $TOTAL_FILES"
echo "Total size: $TOTAL_SIZE"

if command -v xclip >/dev/null 2>&1; then
  xclip -selection clipboard < "$OUTPUT_FILE"
  echo "Contents of $OUTPUT_FILE copied to clipboard."
else
  echo "xclip not found. Please install xclip to copy to clipboard."
fi