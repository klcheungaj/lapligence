// Decision S24-D6: only a string literal argument of a display, write, strobe,
// monitor, file or severity task, or of $swrite, is a format. A string-typed
// argument (a variable, parameter, member, element, function, method or
// conditional result, or a concatenation) prints its text, "%" and "\"
// characters included, and takes no following argument: those print in their
// default formats. The format argument of $sformat and $sformatf is a format
// whatever its type.
//
// IEEE 1800-2009 21.2.1.2 (SystemVerilog-1800-2009.txt L35969-35974):
//   "Each escape sequence, when included in a string literal argument,
//   specifies the display format for a subsequent expression. For each %
//   character (except %m, %l, and %%) that appears in a string literal, a
//   corresponding expression shall follow the string in the argument list."
// 21.3.3 (L36747-36751):
//   "Unlike the display and write family of output system tasks, $sformat
//   always interprets its second argument, and only its second argument, as a
//   format string. This format argument can be a string literal, such as
//   "data is %d", or may be a an expression of integral, unpacked array of
//   byte, or string data types whose content is interpreted as the formatting
//   string."
//
// Industry-standard behaviour interprets any string-typed argument as a
// format; llg keeps the text (user decision).
module tb;
  typedef struct { string s; int i; } rec_t;
  class box;
    string s = "c=%0d";
  endclass
  string f, so, q[$];
  string one = "%d", plain = "text", sa[2];
  rec_t rec;
  box b;
  reg [8*32:1] packed_text;
  int a = 5, fd, code;
  parameter string PS = "p=%0d";
  localparam LS = string'("l=%0h");
  function string fn(); return "fn=%b"; endfunction
  initial begin
    b = new;
    f = "x=%0d";
    rec.s = "m=%s";
    sa[1] = "e=%0d";
    q.push_back("q=%0d");
    // Alone and with following arguments, which keep their default formats.
    $display(f);
    $display(f, a);
    $display(one, 8'd7, 1.5, plain);
    $display("%% ", "%m ", "", f);
    $display(a, f, a);
    $displayh(f, 8'hab);
    $displayb(f, 4'b1x0z);
    $displayo(f, 6'o17);
    $write(f, a, "\n");
    $writeh(f, 8'hcd, "\n");
    // A literal before or after it is still a format, and takes the string as
    // an operand.
    $display("%s", f, a);
    $display("%0d:", a, f, 7);
    $display(f, "%0d", a);
    $display("[%10s]", one);
    // Every kind of string operand.
    $display(rec.s, a);
    $display(b.s, a);
    $display(sa[1], a);
    $display(q[0], a);
    $display(PS, a);
    $display(LS, a);
    $display(fn(), a);
    $display({f, " ", one}, a);
    $display(f.toupper(), a);
    $display(f.substr(1, 3), a);
    $display(a == 5 ? f : one, a);
    $display($sformatf("%%%s", "0d"), a);
    // Escapes are processed in literals only.
    f = {"t\\", "n%%"};
    $display(f);
    f = "x=%0d";
    // String targets.
    $swrite(so, f, a);
    $display("<", so, ">");
    $swriteh(so, f, 8'h1e);
    $display("<", so, ">");
    $swrite(packed_text, f, a);
    $display("<%0s>", packed_text);
    // $sformat and $sformatf interpret the argument.
    $sformat(so, f, a);
    $display("<", so, ">");
    so = $sformatf(f, a);
    $display("<", so, ">");
    $display($sformatf(f, a), a);
    // Only the format argument is interpreted.
    so = $sformatf("%0d %s", a, f);
    $display("<", so, ">");
    so = $sformatf("", f, a);
    $display("<", so, ">");
    // Files.
    fd = $fopen("S24-D6_string_argument_is_text.txt", "w");
    $fwrite(fd, f, a, "\n");
    $fdisplay(fd, f, a);
    $fclose(fd);
    fd = $fopen("S24-D6_string_argument_is_text.txt", "r");
    code = $fgets(so, fd);
    $write("file ", so);
    code = $fgets(so, fd);
    $write("file ", so);
    $fclose(fd);
    if (code == 0) $display("no file text");
    $fdisplay(1, f, a);
    $fstrobe(1, f, a);
    #1;
    // Postponed output.
    $strobe("strobe ", f, a);
    #1;
    $monitor(f, a);
    #1 a = 6;
    #1 f = "y=%0d";
    #1 $monitoroff;
    $finish;
  end
endmodule
