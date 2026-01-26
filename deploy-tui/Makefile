.PHONY: build clean run tidy

build: tidy
	go build -o deploy-tui .

tidy:
	go mod tidy

run: build
	./deploy-tui

clean:
	rm -f deploy-tui
	rm -f ../deploy-tui.go
