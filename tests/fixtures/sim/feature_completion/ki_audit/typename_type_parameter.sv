// $typename of a type parameter inside a class specialization returns the
// resolved type (20.6.1, decision TN-D2).
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
