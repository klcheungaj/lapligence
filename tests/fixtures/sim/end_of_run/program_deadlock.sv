// An initial procedure of a program block counts like one of a module.
module tb;
  event ev;
  program p;
    initial @(ev);
  endprogram
  final $display("final");
endmodule
