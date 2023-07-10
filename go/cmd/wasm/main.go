package main

//import "time"

func main() {
	println("hi")
	//c := make(chan int64, 1)
	//go func() {
	//	for {
	//		time.Sleep(time.Second)
	//		c <- 0 //time.Now().UnixNano()
	//	}
	//}()
	//for {
	//	println("hi", <-c)
	//}
}

//export moontrade.alloc
func malloc(size uintptr) uintptr {
	return 0
}

//export moontrade.realloc
func realloc(ptr uintptr) {
}

//export moontrade.free
func free(ptr uintptr) {
}

// This function is exported to JavaScript, so can be called using
// exports.multiply() in JavaScript.
//
//export multiply
func multiply(x, y int) int {
	return x * y
}
