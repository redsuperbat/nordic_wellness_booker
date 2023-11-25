terraform {
  required_providers {
    kubernetes = {
      source = "hashicorp/kubernetes"
    }
  }
  backend "kubernetes" {
    namespace     = "terraform-backend"
    secret_suffix = "nordic-wellness-booker"
    config_path   = "~/.kube/config"
  }
}

provider "kubernetes" {
  config_path = "~/.kube/config"
}

locals {
  namespace = "rsb-apps"
  name      = "nordic-wellness-booker"
}

data "terraform_remote_state" "rsb_config" {
  backend = "kubernetes"
  config = {
    namespace     = "terraform-backend"
    secret_suffix = "rsb-config"
    config_path   = "~/.kube/config"
  }
}

variable "image_tag" {
  type = string
}


resource "kubernetes_namespace_v1" "nordic_wellness_booker_ns" {
  metadata {
    name = local.name
  }
}


resource "kubernetes_config_map_v1" "config_map" {
  metadata {
    name      = local.name
    namespace = local.namespace
  }
  data = {
    "bookable-activities.json" = file("${path.module}/../assets/bookable-activities.json")
  }
}

resource "kubernetes_cron_job_v1" "nordic_wellness_booker_job" {
  metadata {
    name      = local.name
    namespace = local.namespace
  }


  spec {
    schedule = "*/5 * * * *"
    job_template {
      metadata {
        name = local.name
      }
      spec {
        template {
          metadata {
            labels = {
              app = local.name
            }
          }
          spec {
            container {
              name  = local.name
              image = "maxrsb/nordic_wellness_booker:${var.image_tag}"
              volume_mount {
                name       = kubernetes_config_map_v1.config_map.metadata[0].name
                mount_path = "/app/assets"
              }
              resources {
                requests = {
                  cpu    = "20m"
                  memory = "5Mi"
                }

                limits = {
                  cpu    = "100m"
                  memory = "20Mi"
                }
              }
            }

            volume {
              name = kubernetes_config_map_v1.config_map.metadata[0].name
              config_map {
                name = kubernetes_config_map_v1.config_map.metadata[0].name
              }
            }
          }
        }
      }
    }
  }
}
