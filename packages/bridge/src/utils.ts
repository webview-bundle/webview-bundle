export function snakeCase(str: string): string {
  return str
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2') // camelCase boundary: "fooBar" -> "foo Bar"
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2') // acronym boundary: "HTTPBundle" -> "HTTP Bundle"
    .replace(/[^a-zA-Z0-9]+/g, ' ') // separators ("-", "_", spaces, ...) -> single space
    .trim()
    .replace(/ /g, '_')
    .toLowerCase();
}
