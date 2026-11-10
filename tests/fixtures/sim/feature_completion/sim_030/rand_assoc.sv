// Random associative arrays (IEEE 1800-2009 18.4, 18.5.8.1): randomize draws
// the elements of the entries the array holds and changes no key. A foreach
// index is an entry's key, a select names a key, and reductions walk the
// entries. Lines print relations every legal solution satisfies.
typedef enum bit [2:0] {RED = 3'd1, GREEN = 3'd2, BLUE = 3'd5} color_e;

class keyed_c;
  rand bit [7:0] aa[int];
  rand bit [3:0] by_byte[byte];
  rand logic [7:0] wide[bit [63:0]];
  rand shortint deep[longint];
  rand bit [7:0] nib[bit [3:0]];
  rand color_e paint[int];
  rand int total;
  rand bit [7:0] pick;
  int at;
  constraint c1 { foreach (aa[k]) aa[k] == k + 3; }
  constraint c2 { foreach (by_byte[k]) by_byte[k] < 10; by_byte[-3] == 7; by_byte[at] == 2; }
  constraint c3 { total == aa.sum() with (int'(item)); }
  constraint c4 { foreach (wide[k]) wide[k] == k[7:0] + 8'd1; }
  constraint c5 { foreach (deep[k]) deep[k] == (k < 0 ? -16'sd5 : 16'sd5); }
  constraint c6 { foreach (nib[k]) nib[k] > k; nib.sum() with (int'(item)) == 40; }
  constraint c7 { pick inside {aa}; }
endclass

class free_c;
  rand bit [7:0] aa[int];
endclass

class holder_c;
  rand keyed_c m;
  rand bit [3:0] lim;
  rand bit [3:0] own[int];
  constraint c { foreach (own[k]) own[k] < lim; solve lim before own; }
  function new();
    m = new;
  endfunction
endclass

module tb;
  keyed_c c = new;
  free_c u = new;
  holder_c h = new;
  int ok, bad, changed, sum;
  bit [7:0] old[int];
  bit [7:0] scope[int];
  int lim_hi;

  task automatic fill(keyed_c k);
    k.aa[5] = 0; k.aa[-2] = 0; k.aa[100] = 0;
    k.by_byte[-3] = 0; k.by_byte[4] = 0; k.by_byte[127] = 0;
    k.at = 4;
    k.wide[64'hffff_ffff_ffff_ff02] = 0; k.wide[64'd9] = 0;
    k.deep[-64'sd5_000_000_000] = 0; k.deep[64'sd7] = 0;
    k.nib[4'd1] = 0; k.nib[4'd15] = 0; k.nib[4'd7] = 0;
    k.paint[3] = RED; k.paint[-9] = RED;
  endtask

  function automatic int check(keyed_c k);
    int n;
    foreach (k.aa[i]) if (k.aa[i] != 8'(i + 3)) n++;
    foreach (k.by_byte[i]) if (k.by_byte[i] >= 10) n++;
    if (k.by_byte[-3] != 7 || k.by_byte[k.at] != 2) n++;
    sum = 0;
    foreach (k.aa[i]) sum += k.aa[i];
    if (k.total != sum) n++;
    foreach (k.wide[i]) if (k.wide[i] !== i[7:0] + 8'd1) n++;
    foreach (k.deep[i]) if (k.deep[i] != (i < 64'sd0 ? -16'sd5 : 16'sd5)) n++;
    sum = 0;
    foreach (k.nib[i]) begin
      if (k.nib[i] <= i) n++;
      sum += k.nib[i];
    end
    if (sum != 40) n++;
    foreach (k.paint[i]) if (!(k.paint[i] inside {RED, GREEN, BLUE})) n++;
    if (!k.aa.exists(5) || !(k.pick inside {k.aa})) n++;
    return n;
  endfunction

  initial begin
    fill(c);
    repeat (20) begin
      if (!c.randomize()) bad++;
      bad += check(c);
    end
    $display("keys: bad=%0d aa=%0d %0d %0d entries=%0d %0d %0d %0d %0d %0d", bad, c.aa[-2], c.aa[5],
             c.aa[100], c.aa.num(), c.by_byte.num(), c.wide.num(), c.deep.num(), c.nib.num(),
             c.paint.num());

    // Another set of keys and another state key: the constraints follow the
    // entries the array holds at each call.
    c.aa.delete(5);
    c.aa[40] = 0;
    c.by_byte[-100] = 0;
    c.at = -100;
    repeat (20) begin
      if (!c.randomize()) bad++;
      bad += check_keys();
    end
    $display("other keys: bad=%0d aa=%0d %0d %0d by_byte[-100]=%0d", bad, c.aa[-2], c.aa[40],
             c.aa[100], c.by_byte[-100]);

    // Unconstrained: every element is drawn, no entry appears or goes.
    u.aa[1] = 0; u.aa[9] = 0; u.aa[77] = 0;
    repeat (40) begin
      old = u.aa;
      if (!u.randomize()) bad++;
      foreach (u.aa[k]) if (u.aa[k] != old[k]) changed++;
    end
    $display("unconstrained: bad=%0d entries=%0d keys=%0b%0b%0b changed=%0b", bad, u.aa.num(),
             u.aa.exists(1), u.aa.exists(9), u.aa.exists(77), changed > 100);

    // An inactive array is state; an empty one constrains nothing.
    u.aa.rand_mode(0);
    old = u.aa;
    ok = u.randomize();
    changed = 0;
    foreach (u.aa[k]) if (u.aa[k] != old[k]) changed++;
    $display("inactive: ok=%0d changed=%0d", ok, changed);
    u.aa.rand_mode(1);
    u.aa.delete();
    ok = u.randomize();
    $display("empty: ok=%0d entries=%0d", ok, u.aa.num());

    // A member object's arrays, an order before the elements, and a scope
    // array.
    fill(h.m);
    h.own[2] = 0; h.own[-7] = 0; h.own[1000] = 0;
    repeat (200) begin
      if (!h.randomize()) bad++;
      bad += check(h.m);
      foreach (h.own[k]) if (h.own[k] >= h.lim) bad++;
      if (h.lim >= 8) lim_hi++;
    end
    // lim is drawn first over the values with a legal completion, 1 to 15.
    $display("member and order: bad=%0d lim high=%0b", bad, lim_hi > 70 && lim_hi < 145);
    scope[3] = 0; scope[6] = 0;
    repeat (20) begin
      ok = std::randomize(scope) with { foreach (scope[k]) scope[k] == 2 * k + 1; };
      if (!ok || scope[3] != 7 || scope[6] != 13 || scope.num() != 2) bad++;
    end
    $display("scope: bad=%0d", bad);
    $finish;
  end

  function automatic int check_keys();
    int n;
    foreach (c.aa[i]) if (c.aa[i] != 8'(i + 3)) n++;
    if (c.by_byte[-3] != 7 || c.by_byte[-100] != 2) n++;
    if (c.aa.exists(5) || !c.aa.exists(40) || c.aa.num() != 3) n++;
    return n;
  endfunction
endmodule
