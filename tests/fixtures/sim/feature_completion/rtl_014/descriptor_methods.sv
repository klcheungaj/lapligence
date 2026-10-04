// IEEE 1800-2009 7.12, 11.4.13, 12.7.3, 20.7: methods, membership, foreach
// and queries over descriptor-backed fixed arrays (65,537 cells per row).
// Every operation reads or moves cells in place; none forms one packed value.
module tb;
  localparam int N = 65537;
  bit [31:0] big [N];
  bit [15:0] rows [0:2][1:N];
  int unsigned keyed [N-1:0];
  integer sel;
  int wakes;
  int nonzero;

  always @(big[N-1]) wakes++;

  initial begin
    #1;
    big[0] = 5;
    big[N-1] = 7;
    big[100] = 3;
    big[200] = 3;
    $display("sum=%0d xor=%0d", big.sum(), big.xor());
    $display("with=%0d", big.sum() with (item.index == N - 1 ? item : 0));
    $display("inside=%b %b", 32'd7 inside {big}, 32'd9 inside {big});
    #1;
    big.reverse();
    $display("reverse %0d %0d %0d %0d", big[0], big[N-1], big[N-101], big[N-201]);
    #1;
    big.sort();
    $display("sort %0d %0d %0d %0d %0d", big[N-1], big[N-2], big[N-3], big[N-4], big[N-5]);
    #1;
    big.rsort() with (item ^ 32'd4);
    $display("rsort %0d %0d %0d %0d %0d", big[0], big[1], big[2], big[N-2], big[N-1]);
    nonzero = 0;
    foreach (big[i]) if (big[i] != 0) nonzero++;
    $display("nonzero=%0d", nonzero);

    rows[1][1] = 9;
    rows[1][N] = 4;
    rows[2][3] = 1;
    sel = 1;
    $display("row sum=%0d with=%0d", rows[sel].sum(), rows[sel].sum() with (int'(item) * item.index));
    $display("row inside=%b %b all=%b", 16'd4 inside {rows[sel]}, 16'd1 inside {rows[sel]},
             16'd1 inside {rows});
    rows[sel].sort();
    $display("row sort %0d %0d %0d", rows[1][N-2], rows[1][N-1], rows[1][N]);
    rows.reverse();
    $display("rows reverse %0d %0d %0d", rows[0][3], rows[2][3], rows[1][N]);
    rows[sel].rsort();
    $display("row rsort %0d %0d %0d", rows[1][1], rows[1][2], rows[1][3]);
    sel = 'x;
    rows[sel].reverse();
    $display("unselected %0d %0d %0d", rows[sel].sum(), rows[1][1], rows[1][N]);
    $display("sizes %0d %0d %0d", $size(rows, 2), $size(rows[1]), $left(rows[2]));

    keyed[0] = 3;
    keyed[1] = 2;
    keyed[2] = 1;
    keyed.sort();
    $display("keyed %0d %0d %0d %0d", keyed[N-1], keyed[2], keyed[1], keyed[0]);
    keyed.sort() with (item.index);
    $display("keyed index %0d %0d %0d %0d", keyed[N-1], keyed[N-2], keyed[N-3], keyed[0]);
    #1;
    $display("wakes=%0d", wakes);
    $finish(0);
  end
endmodule
