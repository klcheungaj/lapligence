// SIM-018: an array with class keys orders objects by a creation serial
// (S07-D9). When the serials run out, which the test forces by narrowing them,
// the run stops with a fatal that names the limit; a wrap would reorder keys.
// At the default width the run ends normally.
class item_c;
  int id;
endclass

module tb;
  int table_a[item_c];
  item_c k;

  initial begin
    for (int i = 0; i < 100; i++) begin
      k = new;
      if (i % 40 == 0) table_a[k] = i;
    end
    $display("kept %0d", table_a.num());
    $finish(0);
  end
endmodule
