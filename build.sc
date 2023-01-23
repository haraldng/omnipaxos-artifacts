#!/usr/bin/env amm

import ammonite.ops._
import ammonite.ops.ImplicitWd._
import scala.concurrent.duration._

case class Builder(label: String, env: Path, environment: Map[String, String], exec: Path, args: Seq[Shellable], cleanArgs: Seq[Shellable]) {
        lazy val cmd = Command(Vector.empty, environment, Shellout.executeStream);

	def run(): CommandResult = cmd.applyDynamic(exec.toString)(args: _*)(env);
	def clean(): CommandResult = cmd.applyDynamic(exec.toString)(cleanArgs: _*)(env);
}

def relps(s: String): String = relp(s).toString;
def relp(s: String): Path = (pwd / RelPath(s));

def binp(s: Symbol): Path = {
	val path = sys.env("PATH");
	val paths = path.split(":");
	paths.foreach { p =>
		val bin = Path(p) / s;
		if (exists! bin) {
			return bin
		}
	}
	throw new RuntimeException(s"No binary found for $s in path:\n$path");
}

//val sbt = root / 'usr / 'local / 'bin / 'sbt;
val sbt = binp('sbt); //root / 'usr / 'bin / 'sbt;
val sbtEnv: Map[String, String] = Map.empty;
val cargo = binp('cargo); //root / 'home / 'sario / ".cargo" / 'bin / 'cargo;
//val cargoBuildOpts = Seq[Shellable](RUSTFLAGS="-C target-cpu=native");
val cargoBuildCmd = Seq[Shellable]("build", "--release");// ++ cargoBuildOpts;
val cargoEnv: Map[String, String] = Map("RUSTFLAGS" -> "-C target-cpu=native");
val make = binp('make);
val makeEnv: Map[String, String] = Map.empty;

val builders: List[Builder] = List(
	Builder("Shared Library Scala", relp("shared_scala"), sbtEnv, sbt, Seq("publishLocal"), Seq("clean")),
	Builder("Experiment Runner", relp("runner"), sbtEnv, sbt, Seq("assembly", "publishLocal"), Seq("clean")),
	Builder("Kompact", relp("kompact"), cargoEnv, cargo, cargoBuildCmd, Seq("clean")),
);

@main
def main(clean: Boolean = false, useOnly: String = ""): Unit = {
	val totalStart = System.currentTimeMillis();
	var splitted = for (s <- useOnly.split(" ")) yield s.capitalize
	for (i <- 0 until splitted.size){
		if (splitted(i).equalsIgnoreCase("shared_scala")) splitted(i) = "Shared Library Scala"
		else if (splitted(i).equalsIgnoreCase("runner")) splitted(i) = "Experiment Runner"
	}
	var useOnlyBuilders = for (b <- builders if splitted.contains(b.label)) yield b
	if (useOnlyBuilders.size == 0) useOnlyBuilders = builders
	val nBuilders = useOnlyBuilders.size;
	useOnlyBuilders.zipWithIndex.foreach { case (b, i) =>
		try {
			println(s"Starting build [${i+1}/$nBuilders]: ${b.label}");
			val start = System.currentTimeMillis();
			val cmd = clean match {
				case true => b.clean();
				case false => b.run();
			};
			val end = System.currentTimeMillis();
			val time = FiniteDuration(end-start, MILLISECONDS);
			if (cmd.exitCode == 0) {
				println(s"Finished ${b.label} in ${format(time)}");
			} else {
				println("*** ERRORS ***");
				cmd.err.lines.foreach(println);
				println("*** OUT ***");
				cmd.out.lines.foreach(println);
				Console.err.println(s"Error while building ${b.label}:");
				println(cmd);
			}
		} catch {
			case e: Throwable => e.printStackTrace(Console.err);
		}
	}
	val totalEnd = System.currentTimeMillis();
	val totalTime = FiniteDuration(totalEnd-totalStart, MILLISECONDS);
	println(s"Finished all builds in ${format(totalTime)}");
}

def format(d: FiniteDuration): String = {
	var s = "";
	val m = d.toMinutes;
	var rem = if (m == 0) {
		d
	} else {
		s += s"${m}min ";
		d - FiniteDuration(m, MINUTES)
	};
	val sec = rem.toSeconds;
	rem = if (sec == 0) {
		rem
	} else {
		s += s"${sec}s ";
		rem - FiniteDuration(sec, SECONDS)
	};
	val ms = rem.toMillis;
	s += s"${ms}ms";
	s
}
