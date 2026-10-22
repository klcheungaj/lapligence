// ADV-026 A01 with SystemVerilog element types: strings loaded by
// $sreadmemb/$sreadmemh give the same memory as $readmemb/$readmemh of the
// file holding the strings joined by one newline (decision S26-D16).
//   C  two-state words: x/z digits read as 0 (IEEE 1800-2009 21.4.2)
//   I  signed int words: sign by width; a word with more digits ends the load
//   P  packed-struct words
//   N  enumerated words: member values only
//   M  a two-dimensional memory loaded in row-major order
//   V  a part of a memory selected with a slice
module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } nib_t;
  typedef enum logic [1:0] { RED = 0, GREEN = 1, BLUE = 2 } color_t;

  bit [11:0] fc [0:5];   bit [11:0] sc [0:5];
  int fi [0:3];          int si [0:3];
  nib_t fp [0:2];        nib_t sp [0:2];
  color_t fn [0:2];      color_t sn [0:2];
  logic [3:0] fm [2][3]; logic [3:0] sm [2][3];
  logic [7:0] fv [0:7];  logic [7:0] sv [0:7];
  integer i, j;
  integer bad;

  initial begin
    bad = 0;

    $readmemb("c.bin", fc, 0, 5);
    $sreadmemb(sc, 0, 5, "101010101010 111100001111", "x01 1z1 0000_0001_1111");
    for (i = 0; i < 6; i = i + 1) if (fc[i] !== sc[i]) bad = bad + 1;
    $display("C file %h %h %h %h %h %h", fc[0], fc[1], fc[2], fc[3], fc[4], fc[5]);
    $display("C str  %h %h %h %h %h %h", sc[0], sc[1], sc[2], sc[3], sc[4], sc[5]);

    $readmemh("i.hex", fi, 0, 3);
    $sreadmemh(si, 0, 3, "ffffffff 7fffffff", "123456789 80000000");
    for (i = 0; i < 4; i = i + 1) if (fi[i] !== si[i]) bad = bad + 1;
    $display("I file %0d %0d %0d %0d", fi[0], fi[1], fi[2], fi[3]);
    $display("I str  %0d %0d %0d %0d", si[0], si[1], si[2], si[3]);

    $readmemh("p.hex", fp, 0, 2);
    $sreadmemh(sp, 0, 2, "a5 x1", "0f");
    for (i = 0; i < 3; i = i + 1) if (fp[i] !== sp[i]) bad = bad + 1;
    $display("P file %h %h %h", fp[0], fp[1], fp[2]);
    $display("P str  %h %h %h", sp[0], sp[1], sp[2]);

    $readmemh("n.hex", fn, 0, 2);
    $sreadmemh(sn, 0, 2, "1 2", "0");
    for (i = 0; i < 3; i = i + 1) if (fn[i] !== sn[i]) bad = bad + 1;
    $display("N file %s %s %s", fn[0].name(), fn[1].name(), fn[2].name());
    $display("N str  %s %s %s", sn[0].name(), sn[1].name(), sn[2].name());

    $readmemh("m.hex", fm, 0, 1);
    $sreadmemh(sm, 0, 1, "1 2 3", "4 5 6");
    for (i = 0; i < 2; i = i + 1) for (j = 0; j < 3; j = j + 1) if (fm[i][j] !== sm[i][j]) bad = bad + 1;
    $display("M file %h %h %h %h %h %h", fm[0][0], fm[0][1], fm[0][2], fm[1][0], fm[1][1], fm[1][2]);
    $display("M str  %h %h %h %h %h %h", sm[0][0], sm[0][1], sm[0][2], sm[1][0], sm[1][1], sm[1][2]);

    $readmemh("v.hex", fv[2:4], 2, 4);
    $sreadmemh(sv[2:4], 2, 4, "e1", "e2 e3");
    for (i = 0; i < 8; i = i + 1) if (fv[i] !== sv[i]) bad = bad + 1;
    $display("V file %h %h %h %h %h %h %h %h", fv[0], fv[1], fv[2], fv[3], fv[4], fv[5], fv[6], fv[7]);
    $display("V str  %h %h %h %h %h %h %h %h", sv[0], sv[1], sv[2], sv[3], sv[4], sv[5], sv[6], sv[7]);

    $display("mismatches %0d", bad);
    $finish;
  end
endmodule
