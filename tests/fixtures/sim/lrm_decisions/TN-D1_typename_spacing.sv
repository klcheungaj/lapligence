// Decision TN-D1: the $typename text has exactly one space between two
// keywords or identifiers and none anywhere else: none before or inside a
// packed range (logic signed[4095:0]), before the $ of an unpacked dimension
// (byte unsigned$[0:1]), inside an associative index
// (logic signed[3:0]$[logic signed[7:0]]) or after a parameter comma.
//
// IEEE 1800-2009 20.6.1 (SystemVerilog-1800-2009.txt):
// L34549: "b) The default signing is removed, even if present explicitly in
//   the source."
// L34551: "d) A '$' is used as the placeholder for the name of an anonymous
//   unpacked array."
// L34563: "g) Array ranges are represented as unsized decimal numbers."
// L34564-34565: "h) White space in the source is removed and a single space
//   is added to separate identifiers and keywords from each other."
// The examples write one range with a space (L34578, "bit [2:0]") and one
// without (L34582, "A::bit[9:1]"; L34587, "top.AB_t$[0:9]"); rule h puts the
// space only between identifiers and keywords, and industry-standard output
// has none before a range.
module tb;
  logic signed [4095:0] w, wa [3];
  logic [4095:0] wu;
  logic signed [7:0] wq [$];
  bit signed [3:0][7:0] pb;
  logic signed [0:3] asc;
  logic [7:4] off;
  logic [-1:-4] neg;
  logic [0:0] one;
  logic [1:0][2:0][3:0] p3;
  reg signed [8:1] rs;
  reg plain;
  logic signed ls;
  bit signed [31:0] i32;
  wire signed [3:0] ws;
  wand [2:0] wn;
  integer ig;
  integer unsigned igu;
  byte signed bs;
  byte unsigned bu[2];
  int unsigned iuq[$:3];
  shortint unsigned sd[];
  longint unsigned ll;
  time tq[$];
  real rd[][2];
  shortreal sra[3];
  string sa[int], ss[string];
  int wild[*];
  logic [3:0] a2[2][0:2], rev[3:1];
  logic signed [3:0] am[logic signed [7:0]];
  int dd[][], qa[string][$];
  event ev[2];
  chandle chq[$];
  mailbox #(logic signed [4095:0]) mb;
  initial begin
    $display("%s", $typename(w));
    $display("%s", $typename(wu));
    $display("%s", $typename(wa));
    $display("%s", $typename(wq));
    $display("%s", $typename(pb));
    $display("%s", $typename(asc));
    $display("%s", $typename(off));
    $display("%s", $typename(neg));
    $display("%s", $typename(one));
    $display("%s", $typename(p3));
    $display("%s", $typename(rs));
    $display("%s", $typename(plain));
    $display("%s", $typename(ls));
    $display("%s", $typename(i32));
    $display("%s", $typename(ws));
    $display("%s", $typename(wn));
    $display("%s", $typename(ig));
    $display("%s", $typename(igu));
    $display("%s", $typename(bs));
    $display("%s", $typename(bu));
    $display("%s", $typename(iuq));
    $display("%s", $typename(sd));
    $display("%s", $typename(ll));
    $display("%s", $typename(tq));
    $display("%s", $typename(rd));
    $display("%s", $typename(sra));
    $display("%s", $typename(sa));
    $display("%s", $typename(ss));
    $display("%s", $typename(wild));
    $display("%s", $typename(a2));
    $display("%s", $typename(rev));
    $display("%s", $typename(am));
    $display("%s", $typename(dd));
    $display("%s", $typename(qa));
    $display("%s", $typename(ev));
    $display("%s", $typename(chq));
    $display("%s", $typename(mb));
    // Expressions and data types.
    $display("%s", $typename(wa[0]));
    $display("%s", $typename(a2[0]));
    $display("%s", $typename(pb[1]));
    $display("%s", $typename(w[5:2]));
    $display("%s", $typename(wq[0:1]));
    $display("%s", $typename(signed'(wu)));
    $display("%s", $typename({w, wu}));
    $display("%s", $typename(5'sd3));
    $display("%s", $typename(logic   signed   [ 15 : 0 ]));
    $display("%s", $typename(bit unsigned [4*2-1:0]));
    $display("%s", $typename(int signed));
    $finish;
  end
endmodule
