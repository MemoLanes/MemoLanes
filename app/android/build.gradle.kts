buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:8.12.0")
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
    gradle.projectsEvaluated {
        tasks.withType<JavaCompile>().configureEach {
            // Enable detailed warnings for 'unchecked' and 'deprecation' (uncomment the following lines if needed)
            // options.compilerArgs.addAll(
            //     listOf(
            //         "-Xlint:unchecked",
            //         "-Xlint:deprecation"
            //     )
            // )
            // Suppress warnings about JDK version options
            options.compilerArgs.add("-Xlint:-options")
        }
    }
}

val newBuildDir: Directory = rootProject.layout.buildDirectory.dir("../../build").get()
rootProject.layout.buildDirectory.value(newBuildDir)

subprojects {
    val newSubprojectBuildDir: Directory = newBuildDir.dir(project.name)
    project.layout.buildDirectory.value(newSubprojectBuildDir)
}
subprojects {
    project.evaluationDependsOn(":app")
}

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
