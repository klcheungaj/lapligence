// Automatic string, class handle, chandle and process variables a subroutine
// declares and never names again (a library's leftover locals).
class item;
  int value;
  function new(int v);
    value = v;
  endfunction
  function int doubled();
    string unused_text;
    item unused_item;
    return 2 * value;
  endfunction
  function void copy_from(item rhs);
    item other;
    item key;
    if (rhs == null || !$cast(other, rhs))
      return;
    value = other.value;
  endfunction
endclass

module tb;
  function automatic int bump(int a);
    string unused_text;
    item unused_item;
    chandle unused_pointer;
    process unused_process;
    return a + 1;
  endfunction

  function automatic string label(int a);
    string unused_text;
    string text;
    text = $sformatf("n%0d", a);
    return text;
  endfunction

  task automatic report(int a);
    string unused_text;
    item unused_item;
    $display("report %0d", a);
  endtask

  initial begin : main
    item a, b;
    a = new(3);
    b = new(5);
    $display("doubled %0d", a.doubled());
    a.copy_from(b);
    a.copy_from(null);
    $display("copied %0d", a.value);
    $display("bump %0d", bump(41));
    $display("label %s", label(7));
    report(9);
    $finish;
  end
endmodule
