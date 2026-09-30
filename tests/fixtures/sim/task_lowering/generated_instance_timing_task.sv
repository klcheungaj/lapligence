module worker #(parameter integer ID = 0)
  (output integer count, static_count, plain_count);
  function automatic integer value(input integer c);
    return c + ID;
  endfunction
  function static integer static_value(input integer c);
    return value(c);
  endfunction
  task automatic step(inout integer c);
    #1 c = value(c);
  endtask
  task static static_step(inout integer c);
    #1 c = static_value(c);
  endtask
  task static plain_step(inout integer c);
    c = static_value(c);
  endtask
  task automatic plain_chain(inout integer c);
    plain_step(c);
    plain_step(c);
  endtask
  task automatic chain(inout integer c);
    step(c);
    hierarchical_step(c);
  endtask
  task automatic hierarchical_step(inout integer c);
    step(c);
  endtask
  initial begin
    count = 0;
    static_count = 0;
    plain_count = 0;
    chain(count);
    static_step(static_count);
    static_step(static_count);
    plain_chain(plain_count);
  end
endmodule
module tb;
  integer counts[4], statics[4], plains[4];
  integer nested_count, nested_static, nested_plain;
  for (genvar i = 0; i < 4; i++) begin : g
    worker #(i + 1) u(counts[i], statics[i], plains[i]);
  end
  if (1) begin : outer
    for (genvar j = 0; j < 1; j++) begin : inner
      if (j == 0) begin : selected
        worker #(5) u(nested_count, nested_static, nested_plain);
      end
    end
  end
  initial begin
    #5;
    $display("counts %0d %0d %0d %0d", counts[0], counts[1], counts[2], counts[3]);
    $display("static %0d %0d %0d %0d plain %0d %0d %0d %0d nested %0d %0d %0d",
      statics[0], statics[1], statics[2], statics[3],
      plains[0], plains[1], plains[2], plains[3],
      nested_count, nested_static, nested_plain);
    g[0].u.hierarchical_step(g[0].u.count);
    outer.inner[0].selected.u.hierarchical_step(outer.inner[0].selected.u.count);
    #1;
    $display("hierarchical %0d %0d functions %0d %0d",
      counts[0], nested_count, g[1].u.value(10),
      outer.inner[0].selected.u.static_value(10));
    $finish(0);
  end
endmodule
