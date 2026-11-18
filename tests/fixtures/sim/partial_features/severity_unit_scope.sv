// llg-test-fixture: tests/fixtures/sim/partial_features/severity_unit_scope.sv
// The design context of a severity task in compilation-unit code is `$unit`:
// a method and a static method of a `$unit` class, a method of a class nested
// in it, and a `$unit` function. Package and module code keep their scopes.
`timescale 1ns/1ns
class counter;
  int n;
  class inner;
    function void f();
      $warning("nested %0d", 1);
    endfunction
  endclass
  function void bump();
    n++;
    $error("bad count %0d", n);
  endfunction
  static function void check();
    $info("static check");
  endfunction
endclass
function automatic void unit_fn();
  $error("unit function error");
endfunction
module tb;
  counter c;
  counter::inner i;
  initial begin
    c = new;
    i = new;
    c.bump();
    counter::check();
    i.f();
    unit_fn();
    $error("module error");
    $display("done");
    $finish;
  end
endmodule
