package main

// #include <stdlib.h>
import "C"

import (
	"bytes"
	"unsafe"

	"mvdan.cc/sh/v3/syntax"
)

//export FormatShell
func FormatShell(source *C.char, sourceLen C.size_t) *C.char {
	return FormatShellWithConfig(source, sourceLen, 0)
}

//export FormatShellWithConfig
func FormatShellWithConfig(source *C.char, sourceLen C.size_t, indent C.uint) *C.char {
	// Convert C string to Go string
	goSource := C.GoBytes(unsafe.Pointer(source), C.int(sourceLen))

	// Parse the shell script
	parser := syntax.NewParser()
	file, err := parser.Parse(bytes.NewReader(goSource), "")
	if err != nil {
		// Return copy of original on parse error (caller must free)
		return C.CString(string(goSource))
	}

	// Format the parsed syntax tree with config
	var buf bytes.Buffer
	printer := syntax.NewPrinter(syntax.Indent(uint(indent)))
	if err := printer.Print(&buf, file); err != nil {
		// Return copy of original on print error (caller must free)
		return C.CString(string(goSource))
	}

	// Convert formatted result back to C string (caller must free)
	result := buf.String()
	return C.CString(result)
}

//export FreeString
func FreeString(str *C.char) {
	C.free(unsafe.Pointer(str))
}

//export FormatShellBatch
func FormatShellBatch(sources **C.char, lengths *C.size_t, count C.size_t) **C.char {
	return FormatShellBatchWithConfig(sources, lengths, count, 0)
}

//export FormatShellBatchWithConfig
func FormatShellBatchWithConfig(sources **C.char, lengths *C.size_t, count C.size_t, indent C.uint) **C.char {
	// Create a slice of Go strings from the C arrays
	goSources := make([][]byte, int(count))
	sourcesSlice := (*[1 << 28]*C.char)(unsafe.Pointer(sources))[:count]
	lengthsSlice := (*[1 << 28]C.size_t)(unsafe.Pointer(lengths))[:count]

	for i := 0; i < int(count); i++ {
		goSources[i] = C.GoBytes(unsafe.Pointer(sourcesSlice[i]), C.int(lengthsSlice[i]))
	}

	// Format each source
	results := make([]string, int(count))
	parser := syntax.NewParser()
	printer := syntax.NewPrinter(syntax.Indent(uint(indent)))

	for i, src := range goSources {
		file, err := parser.Parse(bytes.NewReader(src), "")
		if err != nil {
			// Return original on parse error
			results[i] = string(src)
			continue
		}

		var buf bytes.Buffer
		if err := printer.Print(&buf, file); err != nil {
			// Return original on print error
			results[i] = string(src)
			continue
		}

		results[i] = buf.String()
	}

	// Convert results back to C array
	cResults := C.malloc(C.size_t(count) * C.size_t(unsafe.Sizeof(unsafe.Pointer(nil))))
	cResultsSlice := (*[1 << 28]*C.char)(unsafe.Pointer(cResults))[:count]

	for i, result := range results {
		cResultsSlice[i] = C.CString(result)
	}

	return (**C.char)(cResults)
}

//export FreeStringArray
func FreeStringArray(arr **C.char, count C.size_t) {
	slice := (*[1 << 28]*C.char)(unsafe.Pointer(arr))[:count]
	for i := 0; i < int(count); i++ {
		C.free(unsafe.Pointer(slice[i]))
	}
	C.free(unsafe.Pointer(arr))
}

func main() {
	// Required for c-shared buildmode
}
