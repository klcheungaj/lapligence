// Adapted FND-002 witness sort_ties (L-F07-13-01, SV2009 7.12.2). The order
// of equal keys is unspecified (allowed results "7 8" and "8 7"), so this
// prints only what every allowed permutation shares: the key at each
// position and the multiset of the non-key field. The 20-element array uses
// the cell-wise path; the pair uses the straight-line schedule.
module tb;
  typedef struct packed {
    bit [7:0] key;
    bit [7:0] id;
  } T;
  T a [2];
  T b [0:19];
  initial begin
    a = '{'{1, 7}, '{1, 8}};
    a.sort() with (item.key);
    $display("%0d %0d %0d", a[0].key, a[1].key, a[0].id + a[1].id);
    foreach (b[i]) b[i] = '{key: 8'(i % 3), id: 8'(i)};
    b.sort() with (item.key);
    $display("%0d %0d %0d %0d", b[0].key, b[6].key, b[7].key, b[19].key);
    $display("%0d", b.sum() with (int'(item.id)));
    $finish(0);
  end
endmodule
