// SIM-022-A02: let bodies bind free names and default actuals where the let
// is declared; actuals bind where the let is used; formals shadow outer names;
// nested lets, package lets and compilation-unit lets follow the same rule.
let unit_plus(a) = a + 1;
package lets;
  int scale = 3;
  let scaled(a) = a * scale;
endpackage
module tb;
  import lets::*;
  int x = 10;
  int y = 1;
  int scale = 100;
  let add_x(a) = a + x;
  let add_default(a, b = y) = a + b;
  let twice(x) = x * 2;
  let nested(a) = twice(a) + add_x(a);
  let typed(bit [3:0] v) = v;
  logic [3:0] mixed = 4'b1x01;

  task automatic in_task(int k);
    int base = k * 10;
    let plus_base(a) = a + base;
    $display("in_task(%0d): plus_base=%0d", k, plus_base(k));
  endtask

  initial begin
    automatic int x = 1000;
    automatic int y = 50;
    $display("add_x(1)=%0d add_x(x)=%0d", add_x(1), add_x(x));
    $display("add_default(2)=%0d add_default(2, y)=%0d add_default(.a(2), .b(7))=%0d",
             add_default(2), add_default(2, y), add_default(.a(2), .b(7)));
    $display("twice(3)=%0d nested(4)=%0d", twice(3), nested(4));
    $display("scaled(2)=%0d unit_plus(4)=%0d typed=%b", scaled(2), unit_plus(4), typed(mixed));
    begin
      let inner(x) = x * 5;
      let outer_x = x;
      $display("inner(2)=%0d outer_x=%0d", inner(2), outer_x);
    end
    in_task(1);
    in_task(2);
    $finish;
  end
endmodule
