package main

import "time"

func main() {
	println("hi tinygo!")

	go func() {
		for {
			time.Sleep(time.Second)
			println(time.Now().UnixMilli())
		}
	}()

	time.Sleep(time.Hour)
}
