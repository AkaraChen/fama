package main

// #include <stdlib.h>
import "C"

import (
	"bytes"
	"go/format"
	"unsafe"

	"mvdan.cc/sh/v3/syntax"
)

//export FormatShell
func FormatShell(source *C.char, sourceLen C.size_t, indent C.uint) *C.char {
	goSource := C.GoBytes(unsafe.Pointer(source), C.int(sourceLen))

	parser := syntax.NewParser()
	file, err := parser.Parse(bytes.NewReader(goSource), "")
	if err != nil {
		return C.CString(string(goSource))
	}

	var buf bytes.Buffer
	printer := syntax.NewPrinter(syntax.Indent(uint(indent)))
	if err := printer.Print(&buf, file); err != nil {
		return C.CString(string(goSource))
	}

	return C.CString(buf.String())
}

//export FormatShellBatch
func FormatShellBatch(sources **C.char, lengths *C.size_t, count C.size_t, indent C.uint) **C.char {
	goSources := make([][]byte, int(count))
	sourcesSlice := (*[1 << 28]*C.char)(unsafe.Pointer(sources))[:count]
	lengthsSlice := (*[1 << 28]C.size_t)(unsafe.Pointer(lengths))[:count]

	for i := 0; i < int(count); i++ {
		goSources[i] = C.GoBytes(unsafe.Pointer(sourcesSlice[i]), C.int(lengthsSlice[i]))
	}

	results := make([]string, int(count))
	parser := syntax.NewParser()
	printer := syntax.NewPrinter(syntax.Indent(uint(indent)))

	for i, src := range goSources {
		file, err := parser.Parse(bytes.NewReader(src), "")
		if err != nil {
			results[i] = string(src)
			continue
		}

		var buf bytes.Buffer
		if err := printer.Print(&buf, file); err != nil {
			results[i] = string(src)
			continue
		}

		results[i] = buf.String()
	}

	cResults := C.malloc(C.size_t(count) * C.size_t(unsafe.Sizeof(unsafe.Pointer(nil))))
	cResultsSlice := (*[1 << 28]*C.char)(unsafe.Pointer(cResults))[:count]

	for i, result := range results {
		cResultsSlice[i] = C.CString(result)
	}

	return (**C.char)(cResults)
}

//export FreeString
func FreeString(str *C.char) {
	C.free(unsafe.Pointer(str))
}

//export FreeStringArray
func FreeStringArray(arr **C.char, count C.size_t) {
	slice := (*[1 << 28]*C.char)(unsafe.Pointer(arr))[:count]
	for i := 0; i < int(count); i++ {
		C.free(unsafe.Pointer(slice[i]))
	}
	C.free(unsafe.Pointer(arr))
}

//export FormatGo
func FormatGo(source *C.char, sourceLen C.size_t) *C.char {
	goSource := C.GoBytes(unsafe.Pointer(source), C.int(sourceLen))

	formatted, err := format.Source(goSource)
	if err != nil {
		// Return original source on error
		return C.CString(string(goSource))
	}

	return C.CString(string(formatted))
}

//export FormatGoBatch
func FormatGoBatch(sources **C.char, lengths *C.size_t, count C.size_t) **C.char {
	goSources := make([][]byte, int(count))
	sourcesSlice := (*[1 << 28]*C.char)(unsafe.Pointer(sources))[:count]
	lengthsSlice := (*[1 << 28]C.size_t)(unsafe.Pointer(lengths))[:count]

	for i := 0; i < int(count); i++ {
		goSources[i] = C.GoBytes(unsafe.Pointer(sourcesSlice[i]), C.int(lengthsSlice[i]))
	}

	results := make([]string, int(count))

	for i, src := range goSources {
		formatted, err := format.Source(src)
		if err != nil {
			results[i] = string(src)
			continue
		}
		results[i] = string(formatted)
	}

	cResults := C.malloc(C.size_t(count) * C.size_t(unsafe.Sizeof(unsafe.Pointer(nil))))
	cResultsSlice := (*[1 << 28]*C.char)(unsafe.Pointer(cResults))[:count]

	for i, result := range results {
		cResultsSlice[i] = C.CString(result)
	}

	return (**C.char)(cResults)
}

func main() {}
