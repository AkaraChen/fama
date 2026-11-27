package main

// #include <stdlib.h>
import "C"

import (
	"bytes"
	"go/format"
	"path/filepath"
	"strings"
	"unsafe"

	cueformat "cuelang.org/go/cue/format"
	"github.com/bazelbuild/buildtools/build"
	jsonnetfmt "github.com/google/go-jsonnet/formatter"
	"github.com/hashicorp/hcl/v2/hclwrite"
	"github.com/open-policy-agent/opa/v1/ast"
	opaformat "github.com/open-policy-agent/opa/v1/format"
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

//export FormatHcl
func FormatHcl(source *C.char, sourceLen C.size_t) *C.char {
	goSource := C.GoBytes(unsafe.Pointer(source), C.int(sourceLen))
	formatted := hclwrite.Format(goSource)
	return C.CString(string(formatted))
}

//export FormatHclBatch
func FormatHclBatch(sources **C.char, lengths *C.size_t, count C.size_t) **C.char {
	goSources := make([][]byte, int(count))
	sourcesSlice := (*[1 << 28]*C.char)(unsafe.Pointer(sources))[:count]
	lengthsSlice := (*[1 << 28]C.size_t)(unsafe.Pointer(lengths))[:count]

	for i := 0; i < int(count); i++ {
		goSources[i] = C.GoBytes(unsafe.Pointer(sourcesSlice[i]), C.int(lengthsSlice[i]))
	}

	results := make([]string, int(count))

	for i, src := range goSources {
		formatted := hclwrite.Format(src)
		results[i] = string(formatted)
	}

	cResults := C.malloc(C.size_t(count) * C.size_t(unsafe.Sizeof(unsafe.Pointer(nil))))
	cResultsSlice := (*[1 << 28]*C.char)(unsafe.Pointer(cResults))[:count]

	for i, result := range results {
		cResultsSlice[i] = C.CString(result)
	}

	return (**C.char)(cResults)
}

func setErrOut(errOut **C.char, err error) {
	if errOut != nil && err != nil {
		*errOut = C.CString(err.Error())
	}
}

//export FormatStarlark
func FormatStarlark(source *C.char, sourceLen C.size_t, filename *C.char, filenameLen C.size_t, errOut **C.char) *C.char {
	goSource := C.GoBytes(unsafe.Pointer(source), C.int(sourceLen))
	goFilename := string(C.GoBytes(unsafe.Pointer(filename), C.int(filenameLen)))

	base := filepath.Base(goFilename)
	var (
		file *build.File
		err  error
	)
	switch {
	case base == "BUILD" || base == "BUILD.bazel" || strings.HasPrefix(base, "BUILD."):
		file, err = build.ParseBuild(goFilename, goSource)
	case base == "WORKSPACE" || base == "WORKSPACE.bazel" || strings.HasPrefix(base, "WORKSPACE."):
		file, err = build.ParseWorkspace(goFilename, goSource)
	case base == "MODULE.bazel":
		file, err = build.ParseModule(goFilename, goSource)
	case strings.HasSuffix(base, ".bzl"):
		file, err = build.ParseBzl(goFilename, goSource)
	default:
		file, err = build.ParseDefault(goFilename, goSource)
	}
	if err != nil {
		setErrOut(errOut, err)
		return nil
	}

	return C.CString(string(build.Format(file)))
}

//export FormatCue
func FormatCue(source *C.char, sourceLen C.size_t, useTabs C.uint, indentWidth C.uint, errOut **C.char) *C.char {
	goSource := C.GoBytes(unsafe.Pointer(source), C.int(sourceLen))

	var opt cueformat.Option
	if useTabs != 0 {
		opt = cueformat.TabIndent(true)
	} else {
		width := int(indentWidth)
		if width <= 0 {
			width = 4
		}
		opt = cueformat.UseSpaces(width)
	}

	formatted, err := cueformat.Source(goSource, opt)
	if err != nil {
		setErrOut(errOut, err)
		return nil
	}

	return C.CString(string(formatted))
}

//export FormatJsonnet
func FormatJsonnet(source *C.char, sourceLen C.size_t, filename *C.char, filenameLen C.size_t, indentWidth C.uint, errOut **C.char) *C.char {
	goSource := string(C.GoBytes(unsafe.Pointer(source), C.int(sourceLen)))
	goFilename := string(C.GoBytes(unsafe.Pointer(filename), C.int(filenameLen)))

	opts := jsonnetfmt.DefaultOptions()
	if indentWidth > 0 {
		opts.Indent = int(indentWidth)
	}

	formatted, err := jsonnetfmt.Format(goFilename, goSource, opts)
	if err != nil {
		setErrOut(errOut, err)
		return nil
	}

	return C.CString(formatted)
}

//export FormatRego
func FormatRego(source *C.char, sourceLen C.size_t, filename *C.char, filenameLen C.size_t, errOut **C.char) *C.char {
	goSource := C.GoBytes(unsafe.Pointer(source), C.int(sourceLen))
	goFilename := string(C.GoBytes(unsafe.Pointer(filename), C.int(filenameLen)))

	formatted, err := opaformat.SourceWithOpts(goFilename, goSource, opaformat.Opts{
		RegoVersion:   ast.RegoV1,
		ParserOptions: &ast.ParserOptions{RegoVersion: ast.RegoV1},
	})
	if err != nil {
		// Fall back to Rego v0 syntax for legacy policies.
		formattedV0, errV0 := opaformat.SourceWithOpts(goFilename, goSource, opaformat.Opts{
			RegoVersion:   ast.RegoV0,
			ParserOptions: &ast.ParserOptions{RegoVersion: ast.RegoV0},
		})
		if errV0 != nil {
			setErrOut(errOut, err)
			return nil
		}
		formatted = formattedV0
	}

	return C.CString(string(formatted))
}

func main() {}
