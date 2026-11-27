// Set membership items outside packed storage: a class handle set with a
// member that calls a function.
module tb;
  class c;
  endclass
  c h, g;
  function automatic c mk();
    mk = new;
  endfunction
  initial begin
    h = new;
    $display("%0d", h inside {g, mk()});
  end
endmodule
