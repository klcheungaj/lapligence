// C code that goes on after the action taken in its exported function or task.
`timescale 1ns/1ns
module tb;
  import "DPI-C" context task c_task(input string act, output int o);
  import "DPI-C" context function int c_func(input string act);
  export "DPI-C" task sv_task;
  export "DPI-C" function sv_func;
  export "DPI-C" function sv_note;
  string act, ctx;
  process victim_p;
  int seen, notes;
  function automatic void take(string a);
    case (a)
      "disable": disable tb.victim;
      "kill": begin process p; p = process::self(); p.kill(); end
      "killv": victim_p.kill();
      "finish": $finish;
      "fatal": $fatal(1, "fatal reached");
      default: ;
    endcase
    seen++;
  endfunction
  task automatic sv_task(input string a, output int o);
    o = 7;
    $display("sv_task take"); take(a); $display("sv_task after");
  endtask
  function automatic int sv_func(input string a);
    $display("sv_func take"); take(a); $display("sv_func after");
    return 5;
  endfunction
  function automatic int sv_note(input int n);
    string s;
    int q[$];
    s = $sformatf("note %0d", n);
    q = '{n, n + 1};
    q.push_back(3);
    notes += n;
    $display("%s %0d", s, q.sum());
    return notes;
  endfunction
  task body;
    int o;
    case (ctx)
      "task": begin c_task(act, o); $display("task o=%0d", o); end
      "func": begin o = c_func(act); $display("func o=%0d", o); end
      "expr": begin o = 1 + c_func(act) + c_func("none"); $display("expr o=%0d", o); end
      default: ;
    endcase
  endtask
  task victim;
    victim_p = process::self();
    #1;
    body;
    $display("victim ends");
  endtask
  initial begin
    void'($value$plusargs("act=%s", act));
    void'($value$plusargs("ctx=%s", ctx));
    fork victim; join_none
    #30 $display("status=%s seen=%0d notes=%0d", victim_p.status().name(), seen, notes);
    $finish;
  end
  final begin
    if (ctx == "final") begin int r; r = c_func(act); $display("final r=%0d", r); end
    $display("final seen=%0d notes=%0d", seen, notes);
  end
endmodule
