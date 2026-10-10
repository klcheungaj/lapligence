// AB-O1: how often, and in which order, an array method evaluates its
// `with` expression.
//
// IEEE 1800-2009 7.12 (L9255): "If the expression contained in the with
// clause includes any side effects, the results may be unpredictable."
// 7.12.1 (L9267): "Array locator methods traverse the array in an
// unspecified order."
//
// Decision (llg policy; the standard leaves it open, so another simulator
// may print a different trace): every locator and reduction evaluates the
// `with` expression exactly once per element, in index order, and in key
// order for associative arrays. Portable oracles print only the results.
module tb;
  int q[$] = '{4, 9, 2, 9, 7};
  int a[string];
  int r[$];
  int total;
  string trace;

  function automatic int probe(int value);
    trace = {trace, $sformatf(" %0d", value)};
    return value;
  endfunction

  function automatic int key(string k);
    trace = {trace, " ", k};
    return 1;
  endfunction

  initial begin
    trace = "";
    r = q.find with (probe(item) > 5);
    $display("find %0d:%s", r.size(), trace);
    trace = "";
    total = q.sum() with (probe(item));
    $display("sum %0d:%s", total, trace);
    trace = "";
    r = q.max() with (probe(item));
    $display("max %0d:%s", r[0], trace);
    a["zed"] = 1;
    a["amy"] = 2;
    a["kim"] = 3;
    trace = "";
    total = a.sum() with (key(item.index()));
    $display("assoc %0d:%s", total, trace);
    $finish(0);
  end
endmodule
