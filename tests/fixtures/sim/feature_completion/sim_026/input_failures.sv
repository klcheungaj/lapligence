// SIM-026 A02: independent results for matching failures, input failures
// at end of input, malformed tokens, overlong fields, short binary reads and
// zero-length reads. A conversion that fails leaves its destination and every
// later destination unchanged; the offending character stays unread
// (IEEE 1800-2009 21.3.4.3, 21.3.4.4).
module tb;
  integer fd, c, ch;
  logic [7:0] a, b, d;
  logic [3:0] n;
  logic [8*3-1:0] p;
  logic [31:0] u;
  logic [7:0] m[0:3];
  real r;
  string s;
  initial begin
    a = 8'd1;
    b = 8'd2;
    d = 8'd3;
    c = $sscanf("5 q 7", "%d %d %d", a, b, d);
    $display("A c=%0d a=%0d b=%0d d=%0d", c, a, b, d);
    c = $sscanf("", "%d", a);
    $display("B c=%0d a=%0d", c, a);
    c = $sscanf("   ", "%d", a);
    $display("C c=%0d a=%0d", c, a);
    c = $sscanf("9", "%d %d", a, b);
    $display("D c=%0d a=%0d b=%0d", c, a, b);
    c = $sscanf("x=4", "y=%d", a);
    $display("E c=%0d a=%0d", c, a);
    c = $sscanf("- 3", "%d", a);
    $display("F c=%0d a=%0d", c, a);
    r = 0.5;
    c = $sscanf("e5", "%f", r);
    $display("G c=%0d r=%f", c, r);
    c = $sscanf("300 fff 1011101 abcdef", "%d %h %b %s", a, n, d, p);
    $display("H c=%0d a=%0d n=%h d=%b p=%s", c, a, n, d, p);
    c = $sscanf("99999999999999999999999 1e999 -1e999", "%d %f", a, r);
    $display("I c=%0d a=%0d r=%f", c, a, r);
    c = $sscanf("-1e999", "%e", r);
    $display("J c=%0d r=%f", c, r);
    fd = $fopen("failures.txt", "w");
    $fwrite(fd, "1 2 zz");
    $fclose(fd);
    fd = $fopen("failures.txt", "r");
    c = $fscanf(fd, "%d %d %d", a, b, d);
    ch = $fgetc(fd);
    $display("K c=%0d a=%0d b=%0d d=%b ch=%0d", c, a, b, d, ch);
    s = "keep";
    c = $fscanf(fd, "%s", s);
    $display("L c=%0d s=%s eof=%0d", c, s, $feof(fd) != 0);
    $fclose(fd);
    fd = $fopen("short.bin", "wb");
    $fwrite(fd, "%c%c%c", 8'h41, 8'h42, 8'h43);
    $fclose(fd);
    fd = $fopen("short.bin", "rb");
    u = 32'h11223344;
    c = $fscanf(fd, "%u", u);
    $display("M c=%0d u=%h", c, u);
    c = $fscanf(fd, "%u", u);
    $display("N c=%0d u=%h", c, u);
    $fclose(fd);
    fd = $fopen("short.bin", "rb");
    c = $fread(m, fd, 0, 0);
    $display("O c=%0d tell=%0d m0=%h", c, $ftell(fd), m[0]);
    c = $fread(m, fd, 4);
    $display("P c=%0d tell=%0d", c, $ftell(fd));
    c = $fread(m, fd, 2);
    $display("Q c=%0d m=%h %h %h %h", c, m[0], m[1], m[2], m[3]);
    c = $fread(m, fd);
    $display("R c=%0d m0=%h", c, m[0]);
    $fclose(fd);
    $finish;
  end
endmodule
