// ADV-026 A01: strings loaded by $sreadmemh/$sreadmemb give the same memory
// as $readmemh/$readmemb of the equivalent file image (IEEE 1364-2001 C.13,
// IEEE 1800-2009 D.14). The image of a call is its strings joined by one
// newline (decision S26-D16); the matching data file holds that image.
//   A  ascending bounds, comments, underscore, x/z digits, `@` address
//   B  descending bounds loaded from the highest address downward, one `@`
//   F  start above finish inside an ascending memory; one word too many
//   G  `@` addresses inside the range, backward jump
//   D  100-bit words: short word, all ones, leftmost x and trailing z
//   E  200-bit words
//   H  binary radix with x/z digits
// Every memory is printed (file load, then string load) and compared with !==.
module tb;
  reg [7:0] fa [0:7];   reg [7:0] sa [0:7];
  reg [7:0] fb [7:0];   reg [7:0] sb [7:0];
  reg [7:0] ff [0:7];   reg [7:0] sf [0:7];
  reg [7:0] fg [0:7];   reg [7:0] sg [0:7];
  reg [99:0] fd [0:3];  reg [99:0] sd [0:3];
  reg [199:0] fe [0:1]; reg [199:0] se [0:1];
  reg [5:0] fh [0:3];   reg [5:0] sh [0:3];
  integer i;
  integer bad;

  initial begin
    bad = 0;

    $readmemh("a.hex", fa, 0, 7);
    $sreadmemh(sa, 0, 7, "10 20 3x // line comment", "@5 zZ 6F_", "7a");
    for (i = 0; i < 8; i = i + 1) if (fa[i] !== sa[i]) bad = bad + 1;
    $display("A file %h %h %h %h %h %h %h %h", fa[0], fa[1], fa[2], fa[3], fa[4], fa[5], fa[6], fa[7]);
    $display("A str  %h %h %h %h %h %h %h %h", sa[0], sa[1], sa[2], sa[3], sa[4], sa[5], sa[6], sa[7]);

    $readmemh("b.hex", fb, 7, 0);
    $sreadmemh(sb, 7, 0, "1 2 3", "4 5", "@2 a b c");
    for (i = 0; i < 8; i = i + 1) if (fb[i] !== sb[i]) bad = bad + 1;
    $display("B file %h %h %h %h %h %h %h %h", fb[7], fb[6], fb[5], fb[4], fb[3], fb[2], fb[1], fb[0]);
    $display("B str  %h %h %h %h %h %h %h %h", sb[7], sb[6], sb[5], sb[4], sb[3], sb[2], sb[1], sb[0]);

    $readmemh("f.hex", ff, 5, 2);
    $sreadmemh(sf, 5, 2, "a b", "c d e");
    for (i = 0; i < 8; i = i + 1) if (ff[i] !== sf[i]) bad = bad + 1;
    $display("F file %h %h %h %h %h %h %h %h", ff[0], ff[1], ff[2], ff[3], ff[4], ff[5], ff[6], ff[7]);
    $display("F str  %h %h %h %h %h %h %h %h", sf[0], sf[1], sf[2], sf[3], sf[4], sf[5], sf[6], sf[7]);

    $readmemh("g.hex", fg, 2, 5);
    $sreadmemh(sg, 2, 5, "@4 44 55", "@2 22 33");
    for (i = 0; i < 8; i = i + 1) if (fg[i] !== sg[i]) bad = bad + 1;
    $display("G file %h %h %h %h %h %h %h %h", fg[0], fg[1], fg[2], fg[3], fg[4], fg[5], fg[6], fg[7]);
    $display("G str  %h %h %h %h %h %h %h %h", sg[0], sg[1], sg[2], sg[3], sg[4], sg[5], sg[6], sg[7]);

    $readmemh("d.hex", fd, 0, 3);
    $sreadmemh(sd, 0, 3, "123456789abcdef0123456789 f", "@3 x23456789abcdef012345678z");
    for (i = 0; i < 4; i = i + 1) if (fd[i] !== sd[i]) bad = bad + 1;
    $display("D file %h %h %h %h", fd[0], fd[1], fd[2], fd[3]);
    $display("D str  %h %h %h %h", sd[0], sd[1], sd[2], sd[3]);

    $readmemh("e.hex", fe, 0, 1);
    $sreadmemh(se, 0, 1, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefde", "1");
    for (i = 0; i < 2; i = i + 1) if (fe[i] !== se[i]) bad = bad + 1;
    $display("E file %h %h", fe[0], fe[1]);
    $display("E str  %h %h", se[0], se[1]);

    $readmemb("h.bin", fh, 0, 3);
    $sreadmemb(sh, 0, 3, "000001 1x10_10", "@3 z");
    for (i = 0; i < 4; i = i + 1) if (fh[i] !== sh[i]) bad = bad + 1;
    $display("H file %b %b %b %b", fh[0], fh[1], fh[2], fh[3]);
    $display("H str  %b %b %b %b", sh[0], sh[1], sh[2], sh[3]);

    $display("mismatches %0d", bad);
    $finish;
  end
endmodule
