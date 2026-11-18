// Elements of an unpacked array property of a computed class handle, read and
// written with the handle evaluated once per operation (IEEE 1800-2009 8.4,
// 11.4.1). Formerly listed as rejected in docs/known_issues.md.
module tb;
  class C;
    int a[3];
  endclass
  C h = new;
  C q[$];
  int calls = 0;
  function automatic C get();
    calls++;
    return h;
  endfunction
  initial begin
    h.a[0] = 3;
    h.a[1] = 5;
    $display("%0d", get().a[1]);
    get().a[2] = 7;
    $display("%0d %0d", h.a[2], calls);
    get().a[1] += 2;
    $display("%0d %0d", h.a[1], calls);
    q.push_back(h);
    $display("%0d %0d", q.pop_front().a[0], q.size());
    $finish(0);
  end
endmodule
