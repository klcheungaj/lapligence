// Decision S07-D19: an element of a queue or dynamic array that is a fixed
// unpacked array has the index rules of its cells. With `simulator.
// index_checks` on (the default), an outer (queue or dynamic-array) index
// that selects no element stops the run, whether a cell (`da[k][j]`) or the
// whole element (`row = da[k]`, `da[k] = row`, `da[k] = '{...}`) is read or
// written, as industry-standard behaviour does; with it off
// (`--no-index-checks`) the text applies: the read gives the default and the
// write does nothing. An inner (fixed) index out of range follows the text
// either way, a write one past the end of a queue appends an element of
// defaults, and a missing associative key reads the default and creates no
// entry. `+case=N` runs case N, no plusarg runs them all.
//
// IEEE 1800-2009 7.4.6 (SystemVerilog-1800-2009.txt L8234-8238): "If an index
// expression is out of the address bounds or if any bit in the address is X or
// Z, then the index shall be invalid. The result of reading from an array with
// an invalid index shall return the default uninitialized value for the array
// element type. Writing to an array with an invalid index shall perform no
// operation." 7.10.1 (L9045-9047): "writing to Q[$+1] is legal."
module tb;
  int da[][4];
  int q[$][4];
  int aa[string][4];
  int row[4] = '{1, 2, 3, 4};
  int c, x, far = 7;
  integer xi = 'x;
  function automatic bit run(int n);
    return c == 0 || c == n;
  endfunction
  initial begin
    if (!$value$plusargs("case=%d", c)) c = 0;
    da = new[2];
    q.push_back(row);
    if (run(1)) begin x = da[5][2]; $display("1 da[5][2] reads %0d", x); end
    if (run(2)) begin da[5][2] = 9; $display("2 da[5][2] = 9: size %0d", da.size()); end
    if (run(3)) begin x = q[3][0]; $display("3 q[3][0] reads %0d", x); end
    if (run(4)) begin q[3][0] = 4; $display("4 q[3][0] = 4: size %0d", q.size()); end
    if (run(5)) begin row = da[5]; $display("5 row = da[5]: %p", row); end
    if (run(6)) begin da[5] = row; $display("6 da[5] = row: %p", da); end
    if (run(7)) begin da[5] = '{5, 6, 7, 8}; $display("7 da[5] = pattern: %p", da); end
    if (run(8)) begin x = da[xi][0]; $display("8 da[x][0] reads %0d", x); end
    if (c == 0) begin
      // Without index checks, and followed by the text either way.
      x = da[0][far]; da[1][far] = 3;
      $display("inner %0d %p", x, da);
      q[1][2] = 6;
      $display("append size %0d %p", q.size(), q);
      x = aa["zz"][1];
      $display("missing %0d num %0d", x, aa.num());
    end
    $finish;
  end
endmodule
