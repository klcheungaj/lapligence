// SIM-026 A01/A02: $fgets into string and packed destinations (a packed
// destination holds at most its width in bytes, right-justified), $fgetc,
// $ungetc, a final line without a newline, end of file, and $ftell, $fseek
// and $rewind (IEEE 1800-2009 21.3.4.1, 21.3.4.2, 21.3.5, 21.3.6).
// line_input.txt is "first line\nsecond\n\nlast" (23 bytes, no final newline).
// It is opened with "rb": byte positions are only defined without the
// newline mapping of text mode (21.3.1), which Windows applies.
module tb;
  integer fd, c, c2, ch, pos;
  string s;
  logic [8*4-1:0] narrow;
  logic [8*12-1:0] wide;
  initial begin
    fd = $fopen("line_input.txt", "rb");
    c = $fgets(s, fd);
    $display("A c=%0d same=%0d", c, s == "first line\n");
    c = $fgets(narrow, fd);
    $display("B c=%0d narrow=%s", c, narrow);
    c = $fgets(wide, fd);
    $display("C c=%0d wide=%h", c, wide);
    ch = $fgetc(fd);
    c = $ungetc(65, fd);
    c2 = $fgets(s, fd);
    $display("D ch=%0d u=%0d c=%0d s=%s", ch, c, c2, s);
    c = $fgets(s, fd);
    $display("E c=%0d s=%s eof=%0d", c, s, $feof(fd) != 0);
    pos = $ftell(fd);
    c = $fseek(fd, 11, 0);
    c2 = $fgets(s, fd);
    $display("F pos=%0d seek=%0d c=%0d same=%0d", pos, c, c2, s == "second\n");
    c = $rewind(fd);
    ch = $fgetc(fd);
    $display("G r=%0d ch=%0d tell=%0d", c, ch, $ftell(fd));
    c = $fseek(fd, -2, 2);
    c2 = $fgets(s, fd);
    $display("H seek=%0d c=%0d s=%s", c, c2, s);
    c = $fseek(fd, 1, 1);
    $display("I seek=%0d tell=%0d eof=%0d", c, $ftell(fd), $feof(fd) != 0);
    $fclose(fd);
    $finish;
  end
endmodule
