// S07-D12: `item.index` of a fixed array is the declared index, on a
// descending range too, also for a fixed array that is a subroutine local.
// 7.12.4 L9457-9458: "The index method of an iterator returns the index value
// of the specified dimension."
module tb;
  int d[3:1];
  int e[1:3];
  int u[$];
  function automatic void local_range();
    int l[3:1];
    int r[$];
    l = '{7, 8, 9};
    r = l.find_index with (item.index == 2);
    $display("%0d %0d", r.size(), r[0]);
  endfunction
  initial begin
    d = '{7, 8, 9};
    e = '{7, 8, 9};
    u = d.find_first_index with (item.index == 2);
    $display("%0d %0d", u.size(), u[0]);
    u = e.find_first_index with (item.index == 2);
    $display("%0d %0d", u.size(), u[0]);
    $display("%0d", d.sum with (item.index));
    local_range();
    $finish;
  end
endmodule
