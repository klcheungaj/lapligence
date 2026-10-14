// SIM-027 A01: $writememh/$writememb dump files that $readmemh/$readmemb
// read back exactly (IEEE 1800-2009 21.5, 21.5.1), for ascending, descending,
// negative and nonzero-based ranges, explicit forward and reversed address
// ranges, words wider than 64 bits, two-state words and X/Z digits. Every
// expected value is the stored pattern itself, compared with `!==`.
module tb;
  logic [7:0] asc [0:7], asc_back [0:7];
  logic [7:0] desc [7:0], desc_back [7:0];
  logic [7:0] neg [-3:4], neg_back [-3:4];
  logic [7:0] high [12:5], high_back [12:5];
  logic [99:0] w100 [0:2], w100_back [0:2];
  logic [199:0] w200 [1:0], w200_back [1:0];
  bit [31:0] two [0:3], two_back [0:3];
  logic [5:0] xz [0:3], xz_back [0:3];
  int bad;

  function automatic logic [7:0] pattern(int i);
    return 8'(i * 37 + 5);
  endfunction

  initial begin
    foreach (asc[i]) asc[i] = pattern(i);
    foreach (desc[i]) desc[i] = pattern(i + 10);
    foreach (neg[i]) neg[i] = pattern(i + 20);
    foreach (high[i]) high[i] = pattern(i + 30);
    w100[0] = 100'h1_2345_6789_abcd_ef01_2345_6789;
    w100[1] = {100{1'b1}};
    w100[2] = {36'h0_dead_beef, 64'hx};
    w200[0] = {50'h3_ffff_0000_1111, 150'hz};
    w200[1] = {4'hz, 196'h1};
    foreach (two[i]) two[i] = 32'h8000_0001 << i;
    xz[0] = 6'bx0z1x0;
    xz[1] = 6'bzzzzzz;
    xz[2] = 6'b10x01z;
    xz[3] = 6'bxxxxxx;

    // Whole arrays, both radixes, default (lowest-first) order.
    $writememh("asc.h", asc);
    $writememb("asc.b", asc);
    $writememh("desc.h", desc);
    $writememb("desc.b", desc);
    $writememh("neg.h", neg);
    $writememb("neg.b", neg);
    $writememh("high.h", high);
    $writememb("high.b", high);
    $writememh("w100.h", w100);
    $writememb("w200.b", w200);
    $writememh("two.h", two);
    $writememb("xz.b", xz);
    $writememh("xz.h", xz);

    bad = 0;
    $readmemh("asc.h", asc_back);
    foreach (asc[i]) if (asc_back[i] !== asc[i]) bad++;
    asc_back = '{default: 8'h00};
    $readmemb("asc.b", asc_back);
    foreach (asc[i]) if (asc_back[i] !== asc[i]) bad++;
    $readmemh("desc.h", desc_back);
    foreach (desc[i]) if (desc_back[i] !== desc[i]) bad++;
    desc_back = '{default: 8'h00};
    $readmemb("desc.b", desc_back);
    foreach (desc[i]) if (desc_back[i] !== desc[i]) bad++;
    $readmemh("neg.h", neg_back);
    foreach (neg[i]) if (neg_back[i] !== neg[i]) bad++;
    neg_back = '{default: 8'h00};
    $readmemb("neg.b", neg_back);
    foreach (neg[i]) if (neg_back[i] !== neg[i]) bad++;
    $readmemh("high.h", high_back);
    foreach (high[i]) if (high_back[i] !== high[i]) bad++;
    high_back = '{default: 8'h00};
    $readmemb("high.b", high_back);
    foreach (high[i]) if (high_back[i] !== high[i]) bad++;
    $readmemh("w100.h", w100_back);
    foreach (w100[i]) if (w100_back[i] !== w100[i]) bad++;
    $readmemb("w200.b", w200_back);
    foreach (w200[i]) if (w200_back[i] !== w200[i]) bad++;
    $readmemh("two.h", two_back);
    foreach (two[i]) if (two_back[i] !== two[i]) bad++;
    $readmemb("xz.b", xz_back);
    foreach (xz[i]) if (xz_back[i] !== xz[i]) bad++;
    $display("whole round trips: mismatches=%0d", bad);
    $display("w100[2]=%h w200[0]=%h", w100_back[2], w200_back[0]);
    $display("xz=%b,%b,%b,%b", xz_back[0], xz_back[1], xz_back[2], xz_back[3]);
    // Hex digits of mixed X/Z groups are not exact: a digit whose bits
    // are not all known reads back as X (or Z when all bits are Z).
    xz_back = '{default: 6'b0};
    $readmemh("xz.h", xz_back);
    $display("xz via hex=%b,%b,%b,%b", xz_back[0], xz_back[1], xz_back[2], xz_back[3]);

    // Explicit ranges in both directions, written and read with the same
    // and with the mirrored range.
    $writememh("asc_rev.h", asc, 6, 1);
    asc_back = '{default: 8'h00};
    $readmemh("asc_rev.h", asc_back, 6, 1);
    bad = 0;
    for (int i = 1; i <= 6; i++) if (asc_back[i] !== asc[i]) bad++;
    $display("reversed range same order: mismatches=%0d edges=%h,%h", bad, asc_back[0], asc_back[7]);
    asc_back = '{default: 8'h00};
    $readmemh("asc_rev.h", asc_back, 1, 6);
    bad = 0;
    for (int i = 1; i <= 6; i++) if (asc_back[i] !== asc[7 - i]) bad++;
    $display("reversed range mirrored: mismatches=%0d", bad);
    $writememb("desc_fwd.b", desc, 2, 5);
    desc_back = '{default: 8'h00};
    $readmemb("desc_fwd.b", desc_back, 5, 2);
    bad = 0;
    for (int i = 2; i <= 5; i++) if (desc_back[i] !== desc[7 - i]) bad++;
    $display("descending declaration mirrored: mismatches=%0d edges=%h,%h", bad, desc_back[1], desc_back[6]);
    $writememh("neg_part.h", neg, -2, 0);
    neg_back = '{default: 8'h00};
    $readmemh("neg_part.h", neg_back, 2, 4);
    bad = 0;
    for (int i = 2; i <= 4; i++) if (neg_back[i] !== neg[i - 4]) bad++;
    $display("negative to positive addresses: mismatches=%0d", bad);
    $writememh("high_start.h", high, 9);
    high_back = '{default: 8'h00};
    $readmemh("high_start.h", high_back, 9);
    bad = 0;
    for (int i = 9; i <= 12; i++) if (high_back[i] !== high[i]) bad++;
    $display("start only: mismatches=%0d below=%h", bad, high_back[8]);
    $finish;
  end
endmodule
