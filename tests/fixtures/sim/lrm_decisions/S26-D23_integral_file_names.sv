// S26-D23: a file name or plusarg held in an integral value.
// IEEE 1800-2009 21.3.1 (SystemVerilog-1800-2009.txt L36569-36570):
// "filename is an expression that is a string literal, string data type, or
// an integral data type containing a character string that names the file to
// be opened"; 21.4 (L37248) and 21.7.1.1 (L37757) say the same. The text does
// not say which characters padding adds. Decision: the characters are the
// 8-bit groups of the value from the most significant one; groups of zero are
// left out, wherever they are, and x and z bits read as 0.
module tb;
  reg [8*24-1:0] r;
  bit [8*24-1:0] b;
  reg signed [8*14-1:0] rs;
  integer i, c, x, fd;
  byte by;
  parameter P = "s26d23.dat";
  typedef struct packed { logic [47:0] a; logic [31:0] z; } ps;
  ps st;
  reg [8*10-1:0] arr [0:1];
  logic [7:0] mem [0:3];

  task open(input string tag, input integer f);
    $display("%s %0d", tag, f != 0);
    if (f != 0) $fclose(f);
  endtask

  initial begin
    r = "s26d23.dat"; b = r; rs = r; st = "s26d23.dat"; arr[1] = r;
    fd = $fopen(r);
    $fdisplay(fd, "11 22 33 44");
    $fclose(fd);
    open("reg", $fopen(r, "r"));
    open("bit", $fopen(b, "r"));
    open("signed", $fopen(rs, "r"));
    open("parameter", $fopen(P, "r"));
    open("struct", $fopen(st, "r"));
    open("element", $fopen(arr[1], "r"));
    open("part", $fopen(r[79:0], "r"));
    open("literals", $fopen({"s26d23", ".dat"}, "r"));
    open("concatenation", $fopen({r, 8'h0}, "r"));
    open("expression", $fopen(r | 0, "r"));
    by = "r";
    open("mode", $fopen(r, by));
    open("mode literals", $fopen(r, {"r", "b"}));
    i = "xd23";
    open("integer write", $fopen(i, "w"));
    open("integer read", $fopen("xd23", "r"));
    r = {"s26d23", 8'h0, ".dat"};
    open("zero group", $fopen(r, "r"));
    r[191:184] = 8'hx;
    open("unknown group", $fopen(r, "r"));
    r = 0;
    open("empty", $fopen(r, "r"));

    r = "s26d23.dat";
    $readmemh(r, mem);
    $display("readmemh %h %h", mem[0], mem[3]);
    mem[0] = 0;
    $readmemh({"s26d23", ".dat"}, mem, 0, 0);
    $display("readmemh %h", mem[0]);
    r = "s26d23.out";
    mem[0] = 8'h3c;
    $writememb(r, mem);
    mem[0] = 0;
    $readmemb(r, mem);
    $display("writememb %h", mem[0]);

    r = "12 34";
    b = "%d %d";
    c = $sscanf(r, b, i, x);
    $display("sscanf %0d %0d %0d", c, i, x);

    r = "absent";
    $display("test %0d %0d", $test$plusargs(r), $test$plusargs({"ab", "sent"}));
    r = "absent=%d";
    c = $value$plusargs(r, i);
    $display("value %0d", c);

    r = "s26d23.vcd";
    $dumpfile(r);
    $dumpvars(1, tb);
    #1 $dumpflush;
    open("dump", $fopen("s26d23.vcd", "r"));
    $finish;
  end
endmodule
