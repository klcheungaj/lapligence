// A whole element that is a fixed unpacked array of a dynamic array, queue or
// associative array, read into and written from a fixed-array variable or an
// assignment pattern (`row = da[k]`, `da[k] = row`, `aa[k] = '{...}`), and
// pushed onto a queue. IEEE 1800-2009 7.6 (L8436-8437): "Assignment shall be
// done by assigning each element of the source array to the corresponding
// element of the target array."; 7.8.6 (L8756-8757) and 7.10.1 (L9041-9047):
// an associative write creates the entry and a queue write one past the end
// appends.
module tb;
  int da[][4];
  int q[$][4];
  int aa[string][4];
  real rd[][2];
  int row[4] = '{1, 2, 3, 4};
  int other[4];
  real rr[2] = '{0.5, 1.5};
  int k = 1;
  initial begin
    da = new[2];
    da[k] = row;
    other = da[1];
    $display("da %p other %p", da, other);
    da[0] = '{5, 6, 7, 8};
    row = da[0];
    $display("row %p", row);
    q.push_back(row);
    q[1] = other;
    q[2] = '{9, 9, 9, 9};
    $display("q size %0d %p", q.size(), q);
    aa["b"] = '{5, 6, 7, 8};
    aa["c"] = row;
    $display("aa %p num %0d", aa, aa.num());
    other = aa["b"];
    $display("other %p", other);
    rd = new[1]; rd[0] = rr; rr = '{0.0, 0.0}; rr = rd[0];
    $display("rr %0.1f %0.1f", rr[0], rr[1]);
    $finish;
  end
endmodule
