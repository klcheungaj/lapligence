// S30-D20: randomizing an associative array draws the elements of the entries
// it holds and changes no key: no entry is added or removed. In a constraint a
// foreach index is the key of an entry (in the index type), a select names a
// key (a constant, a loop index or state) and a reduction walks the entries.
// A select of a key the array does not hold fails the call as an index outside
// the array (S30-D11). The size of a random associative array cannot be
// constrained: llg rejects `size()`/`num()` of one in a constraint at compile
// time.
// 18.4: "The size of a dynamic array or queue declared as rand or randc can
// also be constrained." (associative arrays are not named), and "If a dynamic
// array's size is not constrained, then the array shall not be resized and all
// the array elements shall be randomized." 18.5.8.1: "The number of loop
// variables shall not exceed the number of dimensions of the array variable.
// The scope of each loop variable is the foreach constraint construct,
// including its constraint_set. The type of each loop variable is implicitly
// declared to be consistent with the type of array index."
// The keys here are not negative; some simulators mishandle negative keys of
// an associative array in a constraint.
class keyed_c;
  rand bit [7:0] aa[int];
  rand bit [3:0] by_nibble[bit [3:0]];
  rand int total;
  int at;
  constraint c1 { foreach (aa[k]) aa[k] == k + 3; }
  constraint c2 { foreach (by_nibble[k]) by_nibble[k] > k; by_nibble[2] == 9; by_nibble[at] == 12; }
  constraint c3 { total == aa.sum() with (int'(item)); }
endclass

class free_c;
  rand bit [7:0] aa[int];
endclass

module tb;
  keyed_c c = new;
  free_c u = new;
  int bad, changed, sum;
  bit [7:0] old[int];

  function automatic int check();
    int n;
    foreach (c.aa[i]) if (c.aa[i] != i + 3) n++;
    sum = 0;
    foreach (c.aa[i]) sum += c.aa[i];
    if (c.total != sum) n++;
    foreach (c.by_nibble[i]) if (c.by_nibble[i] <= i) n++;
    if (c.by_nibble[2] != 9 || c.by_nibble[c.at] != 12) n++;
    return n;
  endfunction

  initial begin
    c.aa[5] = 0; c.aa[2] = 0; c.aa[100] = 0;
    c.by_nibble[2] = 0; c.by_nibble[7] = 0; c.by_nibble[11] = 0;
    c.at = 7;
    repeat (20) begin
      if (!c.randomize()) bad++;
      bad += check();
    end
    $display("keys: bad=%0d aa=%0d %0d %0d entries=%0d %0d", bad, c.aa[2], c.aa[5], c.aa[100],
             c.aa.num(), c.by_nibble.num());
    // Another key set and another state key.
    c.aa.delete(5);
    c.aa[40] = 0;
    c.at = 11;
    repeat (20) begin
      if (!c.randomize()) bad++;
      bad += check();
    end
    $display("other keys: bad=%0d aa=%0d %0d %0d by_nibble[11]=%0d", bad, c.aa[2], c.aa[40],
             c.aa[100], c.by_nibble[11]);
    // Unconstrained: every element is drawn, no entry appears or goes.
    u.aa[1] = 0; u.aa[9] = 0; u.aa[77] = 0;
    repeat (40) begin
      old = u.aa;
      if (!u.randomize()) bad++;
      foreach (u.aa[k]) if (u.aa[k] != old[k]) changed++;
    end
    $display("unconstrained: bad=%0d entries=%0d keys=%0b%0b%0b changed=%0b", bad, u.aa.num(),
             u.aa.exists(1), u.aa.exists(9), u.aa.exists(77), changed > 100);
    $finish;
  end
endmodule
