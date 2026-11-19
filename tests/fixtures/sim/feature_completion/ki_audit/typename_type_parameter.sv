// $typename of a type parameter inside a class specialization: a known
// difference (docs/known_issues.md, UVM example differences). llg prints the
// spelling where the frontend first met the type; 20.6.1 asks for the
// resolved type. The expected output pins llg's current text so that a fix
// updates it.
package lib;
  class registry #(type T = int);
    static function string name();
      return $typename(T);
    endfunction
  endclass
endpackage
module tb;
  class mine; endclass
  initial begin
    $display("%s", lib::registry#(mine)::name());
    $finish(0);
  end
endmodule
