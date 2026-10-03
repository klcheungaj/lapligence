// SV2009 10.5, 13.4: event controls are timing controls and are illegal in a
// function called by a declaration initializer.
module tb;
  event go;
  function int waits();
    @(go);
    return 1;
  endfunction
  int x = waits();
  initial $finish;
endmodule
