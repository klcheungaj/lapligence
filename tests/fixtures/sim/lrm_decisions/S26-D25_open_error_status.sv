// S26-D25: $ferror after a $fopen that failed.
// IEEE 1800-2009 21.3.7 (SystemVerilog-1800-2009.txt L37202-37204): "A string
// description of the type of error encountered by the most recent file I/O
// operation is written into the variable str, which should be a packed array of
// at least 640 bits wide or a string type. The integral value of the error code
// is returned in errno." L36625-36627: "If a file cannot be opened ... a zero
// is returned ... Applications can call $ferror to determine the cause of the
// most recent error". The text does not name codes. Decision: a failed $fopen
// leaves the error number and text of the host; $ferror of its descriptor, 0,
// gives them. The text is right-justified in an integral destination and a
// narrow one keeps its end.
`timescale 1ns/1ps
module tb;
  integer fd, e;
  string s;
  reg [639:0] wide;
  reg [31:0] narrow;
  initial begin
    fd = $fopen("no_such_directory/none", "r");
    e = $ferror(fd, s); $display("%0d <%s>", e, s);
    e = $ferror(fd, wide); $display("%0d <%0s>", e, wide);
    e = $ferror(fd, narrow); $display("%0d <%s>", e, narrow);
    fd = $fopen("no_such_directory/none");
    e = $ferror(fd, s); $display("%0d <%s>", e, s);
    fd = $fopen("s26d25.tmp", "q");
    e = $ferror(fd, s); $display("%0d", e);
    fd = $fopen("s26d25.tmp", "w");
    s = "kept"; e = $ferror(fd, s); $display("%0d <%s>", e, s);
    s = "kept"; wide = "kept";
    $fclose(fd);
    e = $ferror(fd, s); $display("%0d <%s>", e, s);
    e = $ferror(fd, wide); $display("%0d <%0s>", e, wide);
    $finish;
  end
endmodule
