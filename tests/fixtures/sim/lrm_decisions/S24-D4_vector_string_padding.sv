// Decision S24-D4: %s of a packed vector prints one character per byte of the
// vector, each zero byte as a space wherever it stands, so "ok" in 48 bits
// prints four spaces and "ok". %0s and a left-justified field drop the zero
// bytes, also those inside the text. A vector of at most 8 bits has no field:
// its zero byte prints nothing. x and z bits read as 0. A string prints its
// characters only, and a vector converted to a string loses its zero bytes
// as before (6.16): a file name or a plusargs name gains no spaces.
//
// IEEE 1800-2009 21.2.1.3 (SystemVerilog-1800-2009.txt L36188-36189):
//   "Decimal and string values are expanded with leading spaces while
//   hexadecimal, octal, and binary values use leading zeros."
// 21.2.1.8 (L36453-36455): "If the argument is a variable, its value should
//   be right-justified so that the rightmost bit of the value is the least
//   significant bit of the last character in the string. No termination
//   character or value is required at the end of a string, and leading zeros
//   are never printed."
// The text does not say that the field of a vector is its width; llg follows
// industry-standard behaviour.
module tb;
  reg [47:0] s;
  reg [7:0] c8;
  reg [2:0] c3;
  reg [8:0] c9;
  reg [10:0] w11;
  reg [63:0] mid;
  reg [23:0] tail;
  reg [31:0] xz;
  integer it;
  byte b;
  shortint sh;
  longint ll;
  logic signed [47:0] sg;
  time t;
  reg [199:0] wide;
  reg [8*20:1] ro;
  string so, str, none, f;
  struct packed {logic [15:0] a; logic [23:0] b;} ps;
  union packed {logic [31:0] u; logic [3:0][7:0] v;} pu;

  function string esc(string text);
    esc = "";
    for (int i = 0; i < text.len(); i++) esc = {esc, $sformatf("%h.", text[i])};
  endfunction


  initial begin
    s = "ok";
    $display("[%s][%0s][%1s][%6s][%10s]", s, s, s, s, s);
    $display("[%-s][%-0s][%-1s][%-3s][%-10s]", s, s, s, s, s);
    $display("[%05s][%010s][%-010s]", s, s, s);
    s = 0;
    $display("zero[%s][%0s][%3s][%-3s][%8s]", s, s, s, s, s);
    c8 = "a";
    $display("c8[%s][%0s][%3s][%-3s]", c8, c8, c8, c8);
    c8 = 0;
    $display("c8[%s][%0s][%1s][%2s][%-2s]", c8, c8, c8, c8, c8);
    c8 = 'z;
    $display("c8z[%s][%0s]", c8, c8);
    c3 = 0;
    $display("c3[%s][%2s]", c3, c3);
    c9 = 0;
    $display("c9[%s][%0s][%1s]", c9, c9, c9);
    c9 = "A";
    $display("c9[%s][%0s]", c9, c9);
    w11 = "q";
    $display("w11[%s][%0s]", w11, w11);
    mid = {"a", 8'h0, "b", 8'h0, 8'h0, "cd", 8'h0};
    $display("mid[%s][%0s][%10s][%-10s][%3s]", mid, mid, mid, mid, mid);
    tail = {"a", 16'h0};
    $display("tail[%s][%0s][%5s][%-5s]", tail, tail, tail, tail);
    xz = {8'h0, 8'hxx, "a", 8'bzzzz_zzzz};
    $display("xz[%s][%0s]", xz, xz);
    xz = {8'b0000_000x, "a", 8'b0110_000x, "b"};
    $display("xz[%s][%0s]", xz, xz);
    xz = 'x;
    $display("xz[%s][%0s]", xz, xz);
    it = "ab";
    $display("int[%s][%0s]", it, it);
    it = 0;
    $display("int[%s][%0s]", it, it);
    b = 65;
    sh = "z";
    ll = "long";
    sg = "sg";
    t = "tm";
    $display("[%s][%s][%s][%s][%s]", b, sh, ll, sg, t);
    ps = "abc";
    pu = "p";
    $display("[%s][%0s][%s][%0s]", ps, ps, pu, pu);
    wide = "wide string";
    $display("[%s][%0s][%30s][%-30s]", wide, wide, wide, wide);
    str = "hi";
    none = "";
    $display("str[%s][%0s][%1s][%5s][%-5s][%s][%3s]", str, str, str, str, str, none, none);
    $display("lit[%s][%0s][%5s][%-5s][%s][%3s]", "lit", "lit", "lit", "lit", "", "");
    $display("[%s][%s][%s][%s]", {"a", 8'h0}, {8'h0, "a"}, 65, 16'h41);
    $display("zero in a literal[%s][%0s]", "a\000b", "a\000b");
    s = "ok";
    $display("[%s][%s][%s][%s]", s[23:0], s[15:0], s[19:0], s + 48'd1);
    $display("[%s][%s]", 1'b1 ? s : "abcdef", s == "ok" ? "true" : "false");
    $write("w[%s]\n", s);
    $swrite(so, "[%s]", s);
    $display("swrite %s", so);
    $swrite(ro, "[%s]", s);
    $display("packed[%s][%0s]", ro, ro);
    $sformat(so, "[%s]", s);
    $display("sformat %s", so);
    $display("sformatf %s", $sformatf("[%s][%0s]", s, s));
    f = "[%s][%0s][%8s]";
    $display("run time %s", $sformatf(f, s, s, s));
    so = $sformatf("%s", s);
    $display("%0d %s", so.len(), esc(so));
    so = $sformatf("%0s", s);
    $display("%0d %s", so.len(), esc(so));
    so = string'(s);
    $display("%0d %s", so.len(), esc(so));
    $fdisplay(1, "fdisplay[%s]", s);
    $fwrite(1, "fwrite[%s]\n", s);
    $strobe("strobe[%s]", s);
    $monitor("monitor[%s]", s);
    #1 s = "later";
    #1 $finish;
  end
endmodule
