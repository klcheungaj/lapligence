// The object expression in front of a static property is not evaluated, so
// one that calls a subroutine is rejected (readme: "Object expressions").
module tb;
  class st;
    static int v[2];
    static function int next();
      return 1;
    endfunction
  endclass
  st objects[2];
  function automatic st get();
    return objects[0];
  endfunction
  initial begin
    objects[st::next()].v[0] = 1;
    $display("%0d", get().v[0]);
  end
endmodule
