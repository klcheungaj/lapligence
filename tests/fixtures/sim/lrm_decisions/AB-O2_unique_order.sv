// AB-O2: the order of unique()/unique_index() results and which index a
// duplicate value reports.
//
// IEEE 1800-2009 7.12.1 (L9295-9301): unique() "The queue returned contains
// one and only one entry for each of the values found in the array. The
// ordering of the returned elements is unrelated to the ordering of the
// original array." unique_index(): "... The index returned for duplicate
// valued entries may be the index for one of the duplicates."
//
// Decision (llg policy; the standard leaves it open): results keep the first
// occurrence of each value, in index order. The `sorted` lines are what every
// conforming simulator prints; the `llg` lines show the policy.
module tb;
  string s[$] = '{"pear", "fig", "apple", "fig", "kiwi"};
  int d[] = '{7, 8, 7, 9};
  string u[$];
  int i[$];
  int v[$];

  initial begin
    u = s.unique();
    $display("llg unique %0d: %s %s %s %s", u.size(), u[0], u[1], u[2], u[3]);
    u.sort();
    $display("sorted unique %0d: %s %s %s %s", u.size(), u[0], u[1], u[2], u[3]);
    i = d.unique_index();
    $display("llg unique_index %0d: %0d %0d %0d", i.size(), i[0], i[1], i[2]);
    foreach (i[k]) v.push_back(d[i[k]]);
    v.sort();
    $display("sorted values %0d: %0d %0d %0d", v.size(), v[0], v[1], v[2]);
    $finish(0);
  end
endmodule
