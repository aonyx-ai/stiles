import CodeBlock from "@theme/CodeBlock";

// A line holding only this, after its comment marker, opens a hidden region.
const OPEN = "<";

// And this one closes it.
const CLOSE = ">";

// A line holding this and a name opens a named region, which a page can show
// on its own.
const REGION = "region:";

// And this one closes it.
const END_REGION = "endregion";

// The marker is written as a comment so the file it lives in still compiles
// and still parses. Rust and TOML disagree about which leader that is, and
// this component is pointed at both.
const LEADERS = ["#", "//"];

/**
 * Returns the marker a line carries, or null when it carries none.
 *
 * A marker is { kind } for the hidden markers and the end of a named region,
 * and { kind, name } for the start of a named region.
 */
function marker(line) {
  const text = line.trim();

  for (const leader of LEADERS) {
    if (!text.startsWith(leader)) {
      continue;
    }

    const rest = text.slice(leader.length).trim();
    if (rest === OPEN || rest === CLOSE || rest === END_REGION) {
      return { kind: rest };
    }
    if (rest.startsWith(REGION)) {
      return { kind: REGION, name: rest.slice(REGION.length).trim() };
    }
  }

  return null;
}

/**
 * Returns the lines without the indentation they all share.
 */
function dedent(lines) {
  const indents = lines
    .filter((line) => line.trim() !== "")
    .map((line) => line.length - line.trimStart().length);
  const shared = Math.min(...indents);

  return lines.map((line) => line.slice(shared));
}

/**
 * Returns the lines of the named region, without any marker lines.
 *
 * A name that the file does not hold fails the build, so a page cannot show
 * an empty block after the file drops or renames a region.
 */
function regionLines(lines, name) {
  const shown = [];
  let inside = false;
  let found = false;

  for (const line of lines) {
    const lineMarker = marker(line);

    if (lineMarker?.kind === REGION && lineMarker.name === name) {
      inside = true;
      found = true;
    } else if (lineMarker?.kind === END_REGION) {
      inside = false;
    } else if (inside && lineMarker === null) {
      shown.push(line);
    }
  }

  if (!found || shown.length === 0) {
    throw new Error(`the imported file has no region named "${name}"`);
  }

  return dedent(shown);
}

/**
 * Returns the lines outside the hidden regions, without any marker lines.
 */
function visibleLines(lines) {
  const shown = [];
  let hidden = false;

  for (const line of lines) {
    const found = marker(line);

    if (found?.kind === OPEN) {
      hidden = true;
    } else if (found?.kind === CLOSE) {
      hidden = false;
    } else if (!hidden && found === null) {
      shown.push(line);
    }
  }

  return shown;
}

/**
 * Renders an imported file, minus the regions its markers hide.
 *
 * The page imports the real file, so what a reader copies is what CI
 * compiles. Some of that file is there for the repository rather than for
 * the reader: a manifest comment about workspace membership, say. Wrapping
 * those lines in markers keeps them in the file and off the page.
 *
 * With `region`, the block shows only the named region of the file, so a
 * guide can show one function of an example that CI builds and tests.
 */
export default function FilteredCodeBlock({ children, region, ...props }) {
  const lines = String(children).split("\n");
  const shown =
    region === undefined ? visibleLines(lines) : regionLines(lines, region);

  return <CodeBlock {...props}>{shown.join("\n").trim()}</CodeBlock>;
}
